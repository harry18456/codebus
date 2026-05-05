export type StreamEvent =
  | { kind: 'thought'; text: string }
  | { kind: 'tool_use'; name: string; input: unknown }
  | { kind: 'tool_result'; output: string; isError: boolean }
  | { kind: 'done' }

export type LLMMode = 'ingest' | 'query'

export interface InvokeOptions {
  systemPrompt: string
  userMessage: string
  mode: LLMMode
  cwd: string
  vaultRoot: string
}

export interface LLMProvider {
  invoke(opts: InvokeOptions): AsyncIterable<StreamEvent>
  cancel(): void
}
