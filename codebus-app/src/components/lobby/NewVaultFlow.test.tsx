import { describe, expect, it, vi } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"

import { DetectionDialog } from "./NewVaultFlow"

describe("DetectionDialog", () => {
  it("defaults to Just-Bind selection", () => {
    render(
      <DetectionDialog
        open
        path="/repo"
        onCancel={() => {}}
        onDecide={() => {}}
      />,
    )
    const justBind = screen.getByTestId("just-bind-radio") as HTMLInputElement
    expect(justBind.checked).toBe(true)
    expect(screen.queryByTestId("re-init-confirm-input")).toBeNull()
  })

  it("emits just_bind on confirm when default selection", () => {
    const onDecide = vi.fn()
    render(
      <DetectionDialog open path="/repo" onCancel={() => {}} onDecide={onDecide} />,
    )
    fireEvent.click(screen.getByTestId("detection-confirm"))
    expect(onDecide).toHaveBeenCalledWith({ mode: "just_bind" })
  })

  it("requires typed `delete` before re_init confirm becomes enabled", () => {
    const onDecide = vi.fn()
    render(
      <DetectionDialog open path="/repo" onCancel={() => {}} onDecide={onDecide} />,
    )
    fireEvent.click(screen.getByTestId("re-init-radio"))
    const confirm = screen.getByTestId("detection-confirm") as HTMLButtonElement
    expect(confirm.disabled).toBe(true)
    fireEvent.change(screen.getByTestId("re-init-confirm-input"), {
      target: { value: "delete" },
    })
    expect(confirm.disabled).toBe(false)
    fireEvent.click(confirm)
    expect(onDecide).toHaveBeenCalledWith({ mode: "re_init" })
  })

  it("does not emit anything on Cancel", () => {
    const onCancel = vi.fn()
    const onDecide = vi.fn()
    render(
      <DetectionDialog open path="/repo" onCancel={onCancel} onDecide={onDecide} />,
    )
    fireEvent.click(screen.getByTestId("detection-cancel"))
    expect(onCancel).toHaveBeenCalled()
    expect(onDecide).not.toHaveBeenCalled()
  })

  it("blocks re_init confirm if user types anything other than `delete`", () => {
    const onDecide = vi.fn()
    render(
      <DetectionDialog open path="/repo" onCancel={() => {}} onDecide={onDecide} />,
    )
    fireEvent.click(screen.getByTestId("re-init-radio"))
    fireEvent.change(screen.getByTestId("re-init-confirm-input"), {
      target: { value: "DELETE" }, // wrong case
    })
    const confirm = screen.getByTestId("detection-confirm") as HTMLButtonElement
    expect(confirm.disabled).toBe(true)
    fireEvent.click(confirm)
    expect(onDecide).not.toHaveBeenCalled()
  })
})
