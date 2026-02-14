// Entry point for bundling just-bash into the pctx runtime
// This file is used by `deno bundle` to create a single-file bundle

import { Bash } from "npm:just-bash";

// Re-export the Bash class for use in the runtime
export { Bash };
