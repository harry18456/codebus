"""Knowledge-base support package.

Houses the first-party Qdrant client wrapper (``qdrant_client``) that
runtime code MUST use instead of importing the third-party SDK directly
(see openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
Requirement "Qdrant client wrapper module"), and the Module 2 KB Builder
surface (``KnowledgeBase`` + payload models + chunker) per
openspec/changes/module-2-kb-builder-p0.
"""
from codebus_agent.kb.backend import KBQdrantBackend, QdrantHttpBackend
from codebus_agent.kb.growth_logger import KBGrowthLogger
from codebus_agent.kb.knowledge_base import KnowledgeBase
from codebus_agent.kb.payload import (
    ChunkDraft,
    KBHit,
    KBPayload,
    KBProgressEvent,
    KBStats,
    ProgressCallback,
    ProgressPhase,
    SourceKind,
)

__all__ = [
    "ChunkDraft",
    "KBGrowthLogger",
    "KBHit",
    "KBPayload",
    "KBProgressEvent",
    "KBQdrantBackend",
    "KBStats",
    "KnowledgeBase",
    "ProgressCallback",
    "ProgressPhase",
    "QdrantHttpBackend",
    "SourceKind",
]
