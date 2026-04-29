// SANITIZER_AUDIT_BANNER — single source of the D-015 banner string used
// by SanitizerAuditInspector overlay, /audit/sanitizer standalone page,
// and the AuditPanel sanitize tab sticky header.
//
// Spec: openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md
//   "SanitizerAuditInspector displays a D-015 banner verbatim"
//
// Source-grep invariant: this file is the ONLY place under `web/app/`
// that may contain the substring `raw values are not retained per D-015`.
// All other call sites MUST `import { SANITIZER_AUDIT_BANNER }` from
// here.
export const SANITIZER_AUDIT_BANNER =
  'Audit metadata only · raw values are not retained per D-015.\n' +
  'Placeholder reveal requires a future audit-unlock capability.'
