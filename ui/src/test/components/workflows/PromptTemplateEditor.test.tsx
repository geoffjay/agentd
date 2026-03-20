/**
 * Tests for PromptTemplateEditor component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { PromptTemplateEditor } from '@/components/workflows/PromptTemplateEditor'

describe('PromptTemplateEditor', () => {
  it('renders the textarea', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} />)
    expect(screen.getByRole('textbox')).toBeInTheDocument()
  })

  it('calls onChange when user types', () => {
    const onChange = vi.fn()
    render(<PromptTemplateEditor value="" onChange={onChange} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Hello {{title}}' } })
    expect(onChange).toHaveBeenCalledWith('Hello {{title}}')
  })

  it('shows available variables section', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} />)
    expect(screen.getByText('Available variables')).toBeInTheDocument()
  })

  it('expands variables section when clicked', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} />)
    fireEvent.click(screen.getByText('Available variables'))
    // {{title}} appears in both the variable buttons and the table — use getAllByText
    expect(screen.getAllByText('{{title}}')).not.toHaveLength(0)
    expect(screen.getAllByText('{{body}}')).not.toHaveLength(0)
  })

  it('shows preview button when value is provided', () => {
    render(<PromptTemplateEditor value="Fix: {{title}}" onChange={() => {}} />)
    expect(screen.getByText('Preview with sample data')).toBeInTheDocument()
  })

  it('does not show preview button when value is empty', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} />)
    expect(screen.queryByText('Preview with sample data')).not.toBeInTheDocument()
  })

  it('expands preview when clicked', async () => {
    const { container } = render(
      <PromptTemplateEditor value="Fix: {{title}}" onChange={() => {}} />,
    )
    fireEvent.click(screen.getByText('Preview with sample data'))
    // ShikiHighlighter is async; wait for it to finish rendering the
    // substituted preview text (split across highlight spans).
    await waitFor(() => {
      expect(container.textContent).toMatch(/Fix: Fix login bug/)
    })
  })

  it('renders error message when provided', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} error="Template is required" />)
    expect(screen.getByText('Template is required')).toBeInTheDocument()
  })

  it('applies error border class when error is provided', () => {
    render(<PromptTemplateEditor value="" onChange={() => {}} error="oops" />)
    const textarea = screen.getByRole('textbox')
    expect(textarea.className).toContain('border-red')
  })

  it('inserts variable when variable button is clicked', () => {
    const onChange = vi.fn()
    render(<PromptTemplateEditor value="Start " onChange={onChange} />)
    fireEvent.click(screen.getByText('Available variables'))
    // Click on the {{title}} variable button
    const titleBtn = screen.getAllByText('{{title}}')[0]
    fireEvent.click(titleBtn)
    expect(onChange).toHaveBeenCalledWith('Start {{title}}')
  })
})
