//! Concrete `LlmProvider` implementations. Each impl is a sibling module —
//! `claude_cli` is the day-one impl; future API providers (`anthropic_api`,
//! `openai`, `ollama_local`) land here behind cargo feature gates.
//!
//! Selection happens in [`crate::llm::factory::build_provider`]; callers
//! never reference these submodules directly.

pub mod claude_cli;
