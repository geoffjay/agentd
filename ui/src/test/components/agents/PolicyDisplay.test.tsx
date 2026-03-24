import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { PolicyDisplay } from '@/components/agents/PolicyDisplay'

describe('PolicyDisplay', () => {
  it('renders "Allow All" for allow_all policy', () => {
    render(<PolicyDisplay policy={{ mode: 'allow_all' }} />)
    expect(screen.getByText('Allow All')).toBeInTheDocument()
  })

  it('renders "Deny All" for deny_all policy', () => {
    render(<PolicyDisplay policy={{ mode: 'deny_all' }} />)
    expect(screen.getByText('Deny All')).toBeInTheDocument()
  })

  it('renders "Require Approval" for require_approval policy', () => {
    render(<PolicyDisplay policy={{ mode: 'require_approval' }} />)
    expect(screen.getByText('Require Approval')).toBeInTheDocument()
  })

  it('renders tool badges for allow_list policy', () => {
    render(<PolicyDisplay policy={{ mode: 'allow_list', tools: ['bash', 'read_file'] }} />)
    expect(screen.getByText('Allow List')).toBeInTheDocument()
    expect(screen.getByText('bash')).toBeInTheDocument()
    expect(screen.getByText('read_file')).toBeInTheDocument()
  })

  it('renders tool badges for deny_list policy', () => {
    render(<PolicyDisplay policy={{ mode: 'deny_list', tools: ['rm', 'dd'] }} />)
    expect(screen.getByText('Deny List')).toBeInTheDocument()
    expect(screen.getByText('rm')).toBeInTheDocument()
    expect(screen.getByText('dd')).toBeInTheDocument()
  })

  it('renders "None configured" for empty allow_list', () => {
    render(<PolicyDisplay policy={{ mode: 'allow_list', tools: [] }} />)
    expect(screen.getByText(/none configured/i)).toBeInTheDocument()
  })

  it('renders "None configured" for empty deny_list', () => {
    render(<PolicyDisplay policy={{ mode: 'deny_list', tools: [] }} />)
    expect(screen.getByText(/none configured/i)).toBeInTheDocument()
  })

  it('does not render tools section for allow_all', () => {
    render(<PolicyDisplay policy={{ mode: 'allow_all' }} />)
    expect(screen.queryByText(/none configured/i)).not.toBeInTheDocument()
    expect(screen.queryByText('Tools')).not.toBeInTheDocument()
  })
})
