/**
 * Quiz tab — v1 placeholder.
 *
 * Spec: app-workspace § Workspace Layout and Tab Navigation —
 * "Quiz tab v1 placeholder" entry. The body SHALL render exactly the
 * literal string "Coming soon — quiz flow ships in v3-app-quiz" and
 * no other interactive controls. The real Quiz flow ships in
 * `v3-app-quiz` (capability E in the v3-app roadmap).
 */
export function QuizTab() {
  return (
    <div
      data-testid="quiz-tab"
      className="flex h-full w-full items-center justify-center px-8 text-center"
    >
      <p className="text-[14px] text-fg-secondary">
        Coming soon — quiz flow ships in v3-app-quiz
      </p>
    </div>
  )
}
