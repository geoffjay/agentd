import 'react'

declare module 'react' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface HTMLAttributes<T> {
    xyz?: string
  }
}
