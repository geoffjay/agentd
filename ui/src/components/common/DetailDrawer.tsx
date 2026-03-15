/**
 * DetailDrawer — slide-out drawer for displaying item details.
 *
 * Provides a React Context + hooks pattern so any page can open/close
 * the drawer and render arbitrary children inside it.
 *
 * Uses @animxyz/core for slide-in/slide-out animation from the right.
 *
 * Usage:
 *   <DrawerProvider>
 *     <MyPage />
 *     <DetailDrawer />
 *   </DrawerProvider>
 *
 *   // Inside MyPage:
 *   const { openDrawer, closeDrawer } = useDrawer()
 *   openDrawer('Item Title', <ItemDetails item={item} />)
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { X } from 'lucide-react'

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface DrawerState {
  open: boolean
  title: string
  content: ReactNode | null
}

interface DrawerContextValue {
  /** Whether the drawer is currently open */
  isOpen: boolean
  /** Open the drawer with a title and content */
  openDrawer: (title: string, content: ReactNode) => void
  /** Close the drawer */
  closeDrawer: () => void
}

const DrawerContext = createContext<DrawerContextValue | null>(null)

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export function DrawerProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<DrawerState>({
    open: false,
    title: '',
    content: null,
  })

  const openDrawer = useCallback((title: string, content: ReactNode) => {
    setState({ open: true, title, content })
  }, [])

  const closeDrawer = useCallback(() => {
    setState((prev) => ({ ...prev, open: false }))
  }, [])

  return (
    <DrawerContext.Provider value={{ isOpen: state.open, openDrawer, closeDrawer }}>
      {children}
      <DetailDrawer
        open={state.open}
        title={state.title}
        onClose={closeDrawer}
      >
        {state.content}
      </DetailDrawer>
    </DrawerContext.Provider>
  )
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useDrawer(): DrawerContextValue {
  const ctx = useContext(DrawerContext)
  if (!ctx) {
    throw new Error('useDrawer must be used within a <DrawerProvider>')
  }
  return ctx
}

// ---------------------------------------------------------------------------
// Drawer component
// ---------------------------------------------------------------------------

interface DetailDrawerProps {
  open: boolean
  title: string
  onClose: () => void
  children?: ReactNode
}

function DetailDrawer({ open, title, onClose, children }: DetailDrawerProps) {
  const drawerRef = useRef<HTMLDivElement>(null)
  const [visible, setVisible] = useState(false)
  const [animating, setAnimating] = useState(false)

  // Handle open/close with animation timing
  useEffect(() => {
    if (open) {
      setVisible(true)
      // Small delay to trigger CSS transition after mount
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          setAnimating(true)
        })
      })
    } else {
      setAnimating(false)
      const timer = setTimeout(() => setVisible(false), 300) // match transition duration
      return () => clearTimeout(timer)
    }
  }, [open])

  // Close on Escape key
  useEffect(() => {
    if (!open) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [open, onClose])

  // Focus trap: focus the drawer when it opens
  useEffect(() => {
    if (open && drawerRef.current) {
      drawerRef.current.focus()
    }
  }, [open])

  if (!visible) return null

  return (
    <>
      {/* Backdrop */}
      <div
        className={[
          'fixed inset-0 z-40 bg-black/30 transition-opacity duration-300',
          animating ? 'opacity-100' : 'opacity-0',
        ].join(' ')}
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Drawer panel */}
      <div
        ref={drawerRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        className={[
          'fixed inset-y-0 right-0 z-50 flex w-full max-w-lg flex-col',
          'bg-white shadow-xl dark:bg-gray-900 dark:border-l dark:border-gray-700',
          'transition-transform duration-300 ease-in-out',
          animating ? 'translate-x-0' : 'translate-x-full',
        ].join(' ')}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-200 px-6 py-4 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white truncate">
            {title}
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close drawer"
            className="rounded-md p-1.5 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-300 transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {children}
        </div>
      </div>
    </>
  )
}

export default DetailDrawer
