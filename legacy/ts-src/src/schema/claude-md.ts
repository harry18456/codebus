// Built-in CLAUDE.md schema for the .codebus/ vault. Written to disk by
// `codebus init` (only if missing — never overwrites user customization).
// Schema content lives at codebus-core/src/schema/CLAUDE.md and is read
// at module load time so the same source-of-truth feeds both the TS
// reference impl and the Rust port (Rust uses include_str! over the same
// file). After the rust-rewrite Phase D removes legacy/, only the Rust
// path remains active.
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
// __dirname points to dist/schema/ at runtime (after tsc) and src/schema/
// during vitest (which loads .ts directly). Both sit two levels deep
// relative to project root, so the same '..' walk reaches root.
const schemaPath = resolve(here, '../../codebus-core/src/schema/CLAUDE.md')

export const CODEBUS_SCHEMA_MARKDOWN: string = readFileSync(schemaPath, 'utf8')
