export {
  CURRENT_SCHEMA_VERSION,
  SUPPORTED_VERSIONS,
  SchemaVersionSchema,
  isSupportedVersion,
} from "./versions.js";
export type { SchemaVersion } from "./versions.js";

export {
  migrateTask,
  migrateProject,
  UnsupportedSchemaVersionError,
} from "./migrate.js";

export type { VersionRegistryEntry } from "./registry.js";
