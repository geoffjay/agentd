import { BrowserRouter, Route, Routes } from 'react-router-dom'

function HomePage() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center">
      <h1 className="text-4xl font-bold text-primary-600">agentd</h1>
      <p className="mt-4 text-secondary-500">Web UI — coming soon</p>
    </div>
  )
}

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<HomePage />} />
      </Routes>
    </BrowserRouter>
  )
}

export default App
