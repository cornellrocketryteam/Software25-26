import { BrowserRouter, Routes, Route } from 'react-router-dom';
import LandingPage from './LandingPage';
import PropulsionPage from './PropulsionPage';
import RecoveryPage from './RecoveryPage';


function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<LandingPage />} />
        <Route path="/propulsion" element={<PropulsionPage />} />
        <Route path="/recovery" element={<RecoveryPage />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;