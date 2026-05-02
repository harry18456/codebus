// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Folder picker invocation flow
//     Scenario: User cancels folder picker

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'

const dialogOpenMock = vi.fn()
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (...args: unknown[]) => dialogOpenMock(...args)
}))

import FolderPickerButton from '~/components/workspace-onramp/FolderPickerButton.vue'

beforeEach(() => {
  dialogOpenMock.mockReset()
})

async function flush(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0))
}

describe('<FolderPickerButton>', () => {
  it('renders zh-TW button text "+ 開新 codebase"', () => {
    const wrapper = mount(FolderPickerButton)
    expect(wrapper.text()).toContain('+ 開新 codebase')
  })

  it('emits picked with the absolute path returned by the dialog plugin', async () => {
    dialogOpenMock.mockResolvedValueOnce('/some/path')
    const wrapper = mount(FolderPickerButton)
    await wrapper.find('[data-testid="onramp-folder-picker"]').trigger('click')
    await flush()
    expect(dialogOpenMock).toHaveBeenCalledWith({
      directory: true,
      multiple: false
    })
    const events = wrapper.emitted('picked')
    expect(events).toHaveLength(1)
    expect(events?.[0]).toEqual(['/some/path'])
  })

  it('does NOT emit when the user cancels the dialog (returns null)', async () => {
    dialogOpenMock.mockResolvedValueOnce(null)
    const wrapper = mount(FolderPickerButton)
    await wrapper.find('[data-testid="onramp-folder-picker"]').trigger('click')
    await flush()
    expect(wrapper.emitted('picked')).toBeUndefined()
  })

  it('does NOT emit when the dialog returns an empty string', async () => {
    dialogOpenMock.mockResolvedValueOnce('')
    const wrapper = mount(FolderPickerButton)
    await wrapper.find('[data-testid="onramp-folder-picker"]').trigger('click')
    await flush()
    expect(wrapper.emitted('picked')).toBeUndefined()
  })
})
