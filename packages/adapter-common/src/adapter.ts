import type {
  HippoProject,
  HippoTask,
  PaginatedResult,
  SchemaVersion,
  TaskChangeEvent,
  TaskQuery,
} from "@hippotask/core";
import type { StatusMap } from "./status-map.js";
import type { HippoTaskCreate, HippoTaskUpdate } from "./types.js";

/**
 * Base configuration shared by all adapters.
 */
export interface BaseAdapterConfig {
  /** Status mapping overrides. */
  statusMap?: Partial<StatusMap>;

  /** Request timeout in ms. Default: 30000. */
  timeout?: number;

  /** Maximum retries on transient failures. Default: 3. */
  maxRetries?: number;
}

/**
 * Declares what features an adapter supports.
 * Consumers check capabilities before calling optional methods.
 */
export interface AdapterCapabilities {
  /**
   * HippoTask schema versions this adapter can produce and consume.
   *
   * Adapters committing to lossless round-trips must list every version
   * whose shape they can fully represent. When the registry grows, every
   * adapter declares whether it's been tested against the new version —
   * it's not implicit from the import's resolved version.
   *
   * See [SCHEMA_CHANGELOG](../../core/SCHEMA_CHANGELOG.md) for the
   * current registry, and `docs/SCHEMA.md` §9 for the design rationale.
   */
  supportedSchemaVersions: readonly SchemaVersion[];

  /** Can receive real-time change events. */
  webhooks: boolean;

  /** Supports creating tasks. */
  create: boolean;

  /** Supports updating tasks. */
  update: boolean;

  /** Supports deleting tasks. */
  delete: boolean;

  /** Supports subtasks (parent_id). */
  subtasks: boolean;

  /** Supports custom fields. */
  customFields: boolean;

  /** Supports file attachments. */
  attachments: boolean;

  /** Supports bulk operations. */
  bulk: boolean;
}

/**
 * The adapter interface. Every platform adapter implements this.
 *
 * TConfig is the platform-specific configuration type which must
 * extend BaseAdapterConfig.
 *
 * Follows Interface Segregation: optional methods (onTaskChange)
 * are truly optional. Use `capabilities` to check before calling.
 */
export interface HippoAdapter<
  TConfig extends BaseAdapterConfig = BaseAdapterConfig,
> {
  /** Platform identifier string (e.g. "jira", "linear", "github"). */
  readonly platform: string;

  /** Whether this adapter is currently connected. */
  readonly connected: boolean;

  /** What this adapter supports. */
  readonly capabilities: AdapterCapabilities;

  // ─── Lifecycle ────────────────────────────────────────────

  /** Initialize the adapter with credentials and configuration. */
  connect(config: TConfig): Promise<void>;

  /** Clean up connections and resources. */
  disconnect(): Promise<void>;

  // ─── Task CRUD ────────────────────────────────────────────

  /** Get a single task by its external (platform) ID. */
  getTask(externalId: string): Promise<HippoTask>;

  /** List tasks with optional filters. */
  listTasks(query?: TaskQuery): Promise<PaginatedResult<HippoTask>>;

  /** Create a task on the platform. Returns the task with external_ids populated. */
  createTask(task: HippoTaskCreate): Promise<HippoTask>;

  /** Update a task on the platform. Partial updates supported. */
  updateTask(externalId: string, updates: HippoTaskUpdate): Promise<HippoTask>;

  /** Delete a task on the platform. */
  deleteTask(externalId: string): Promise<void>;

  // ─── Project CRUD ─────────────────────────────────────────

  /** Get a single project by its external ID. */
  getProject(externalId: string): Promise<HippoProject>;

  /** List all accessible projects. */
  listProjects(): Promise<HippoProject[]>;

  // ─── Sync ─────────────────────────────────────────────────

  /**
   * Get tasks that changed since a given timestamp.
   * Used for polling-based sync.
   */
  getChangedTasks(since: string): Promise<HippoTask[]>;

  /**
   * Subscribe to real-time task changes (if platform supports webhooks).
   * Returns an unsubscribe function.
   */
  onTaskChange?(
    callback: (event: TaskChangeEvent) => void,
  ): Promise<() => void>;

  // ─── Mapping ──────────────────────────────────────────────

  /** Get the current status map. */
  getStatusMap(): StatusMap;

  /** Override the default status map (partial overrides merged). */
  setStatusMap(map: Partial<StatusMap>): void;
}
