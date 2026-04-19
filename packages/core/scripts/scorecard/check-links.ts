/**
 * Check that every vendor-doc URL in the scorecard YAMLs is live.
 *
 * This is a **non-CI** script — run it on a schedule (weekly cron in
 * .github/workflows/scorecard-links.yml) or manually before publishing
 * an audit. It is deliberately separate from `pnpm scorecard:validate`
 * because network reachability is flaky and shouldn't block PRs.
 *
 * For every URL found in:
 *   - scores.*.sub_criteria[].evidence
 *   - scores.{schema_flexibility, rate_limits, real_time, data_portability}.evidence
 *   - inline markdown links inside any `note`, `justification`, `bottom_line`, `gotcha`
 *
 * the script does a HEAD (falling back to GET with a 1-byte Range when
 * the server blocks HEAD), retries once on 5xx / network errors, and
 * reports any 4xx/5xx or failed fetch.
 *
 * Exit code: 0 if all live, 1 if any broken.
 * Run: pnpm scorecard:check-links
 */

import { loadProviders } from "./load.js";
import type { ProviderYAML } from "./schema.js";

interface LinkRef {
  readonly url: string;
  readonly source: string;
}

interface LinkResult extends LinkRef {
  readonly ok: boolean;
  readonly status?: number;
  readonly error?: string;
}

const MARKDOWN_LINK_RE = /\[([^\]]+)\]\(([^)]+)\)/g;
const USER_AGENT =
  "Mozilla/5.0 (compatible; HippoTaskScorecardLinkCheck/1.0; +https://github.com/hippotask/hippo-task)";
const TIMEOUT_MS = 8000;
const RETRY_DELAY_MS = 1500;
const CONCURRENCY = 10;

function extractMarkdownUrls(md: string): string[] {
  const urls: string[] = [];
  const re = new RegExp(MARKDOWN_LINK_RE.source, "g");
  for (const match of md.matchAll(re)) {
    const url = match[2];
    if (url && (url.startsWith("http://") || url.startsWith("https://"))) {
      urls.push(url);
    }
  }
  return urls;
}

function collectLinks(provider: ProviderYAML): LinkRef[] {
  const refs: LinkRef[] = [];
  const slug = provider.slug;

  const pushEvidence = (
    section: string,
    evidence: string | undefined,
  ): void => {
    if (evidence)
      refs.push({ url: evidence, source: `${slug}.yaml:${section}` });
  };
  const pushMd = (section: string, text: string): void => {
    for (const url of extractMarkdownUrls(text)) {
      refs.push({ url, source: `${slug}.yaml:${section}` });
    }
  };

  const s = provider.scores;

  const sumDims: Array<
    [string, ReadonlyArray<{ id: string; evidence?: string; note: string }>]
  > = [
    ["api_completeness", s.api_completeness.sub_criteria],
    ["api_ergonomics", s.api_ergonomics.sub_criteria],
    ["auth_simplicity", s.auth_simplicity.sub_criteria],
    ["ai_agent_readiness", s.ai_agent_readiness.sub_criteria],
  ];
  for (const [key, criteria] of sumDims) {
    for (const c of criteria) {
      pushEvidence(`${key}.${c.id}.evidence`, c.evidence);
      pushMd(`${key}.${c.id}.note`, c.note);
    }
  }

  pushEvidence("schema_flexibility.evidence", s.schema_flexibility.evidence);
  pushMd(
    "schema_flexibility.justification",
    s.schema_flexibility.justification,
  );

  pushEvidence("rate_limits.evidence", s.rate_limits.evidence);
  pushMd("rate_limits.justification", s.rate_limits.justification);

  pushEvidence("real_time.evidence", s.real_time.evidence);
  pushMd("real_time.justification", s.real_time.justification);

  pushEvidence("data_portability.evidence", s.data_portability.evidence);
  pushMd("data_portability.justification", s.data_portability.justification);

  pushMd("bottom_line", provider.bottom_line);
  pushMd("gotcha", provider.gotcha);

  return refs;
}

async function probe(
  url: string,
  method: "HEAD" | "GET",
): Promise<{ ok: boolean; status?: number; error?: string }> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const headers: Record<string, string> = { "User-Agent": USER_AGENT };
    if (method === "GET") headers.Range = "bytes=0-0";
    const response = await fetch(url, {
      method,
      redirect: "follow",
      signal: controller.signal,
      headers,
    });
    return { ok: response.ok, status: response.status };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  } finally {
    clearTimeout(timer);
  }
}

/** Some hosts return 403 to scripted clients on both HEAD and GET, even
 *  though the page is live in a real browser (npmjs.com is the main
 *  offender). Swap to a machine-friendly endpoint that mirrors liveness
 *  for the same package. */
function livenessProxy(url: string): string {
  const npmPkg = url.match(
    /^https:\/\/www\.npmjs\.com\/package\/(@?[^/?#]+(?:\/[^/?#]+)?)/,
  );
  if (npmPkg?.[1]) return `https://registry.npmjs.org/${npmPkg[1]}`;
  return url;
}

async function checkUrl(
  url: string,
): Promise<{ ok: boolean; status?: number; error?: string }> {
  const probeUrl = livenessProxy(url);
  // 1) HEAD
  let result = await probe(probeUrl, "HEAD");
  // Some servers block HEAD. Fall back to tiny GET.
  const headBlocked =
    !result.ok &&
    (result.status === 403 || result.status === 405 || result.status === 400);
  if (headBlocked || (!result.ok && result.status === undefined)) {
    result = await probe(probeUrl, "GET");
  }
  // Retry once on transient failure (5xx or network).
  if (!result.ok && (result.status === undefined || result.status >= 500)) {
    await new Promise((r) => setTimeout(r, RETRY_DELAY_MS));
    result = await probe(probeUrl, "GET");
  }
  return result;
}

async function checkAll(links: readonly LinkRef[]): Promise<LinkResult[]> {
  const results: LinkResult[] = [];
  // Dedupe by URL — the same vendor-doc page is often cited from many
  // sub-criteria. Check each unique URL once; fan the result out to all
  // references afterwards.
  const byUrl = new Map<string, LinkRef[]>();
  for (const ref of links) {
    const bucket = byUrl.get(ref.url);
    if (bucket) bucket.push(ref);
    else byUrl.set(ref.url, [ref]);
  }
  const uniqueUrls = [...byUrl.keys()];

  for (let i = 0; i < uniqueUrls.length; i += CONCURRENCY) {
    const batch = uniqueUrls.slice(i, i + CONCURRENCY);
    const batchResults = await Promise.all(
      batch.map(async (url) => ({ url, result: await checkUrl(url) })),
    );
    for (const { url, result } of batchResults) {
      for (const ref of byUrl.get(url) ?? []) {
        results.push({ ...ref, ...result });
      }
    }
    process.stdout.write(
      `\rChecked ${Math.min(i + CONCURRENCY, uniqueUrls.length)}/${uniqueUrls.length} unique URLs…`,
    );
  }
  process.stdout.write("\n");
  return results;
}

async function main(): Promise<void> {
  const providers = loadProviders();
  const refs = providers.flatMap((p) => collectLinks(p.yaml));
  const uniqueCount = new Set(refs.map((r) => r.url)).size;
  console.log(
    `Checking ${refs.length} URL references (${uniqueCount} unique) across ${providers.length} providers…\n`,
  );

  const results = await checkAll(refs);
  const failures = results.filter((r) => !r.ok);
  const uniqueFailureUrls = new Set(failures.map((f) => f.url));

  if (failures.length === 0) {
    console.log(
      `\nAll ${uniqueCount} unique URLs live (${refs.length} citation references total).`,
    );
    return;
  }

  console.error(
    `\nLink check failed: ${uniqueFailureUrls.size} unique URLs unreachable (${failures.length} citation references affected):\n`,
  );
  const failuresByUrl = new Map<string, LinkResult[]>();
  for (const f of failures) {
    const bucket = failuresByUrl.get(f.url);
    if (bucket) bucket.push(f);
    else failuresByUrl.set(f.url, [f]);
  }
  for (const [url, fs] of failuresByUrl) {
    const first = fs[0];
    const status = first?.status
      ? `HTTP ${first.status}`
      : (first?.error ?? "unknown");
    console.error(`  [${status}] ${url}`);
    for (const f of fs) {
      console.error(`    • ${f.source}`);
    }
  }
  process.exit(1);
}

void main();
