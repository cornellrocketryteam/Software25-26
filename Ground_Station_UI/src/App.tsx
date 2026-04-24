import { BrowserRouter, Routes, Route } from 'react-router-dom';
import LandingPage from './LandingPage';
import PropulsionPage from './PropulsionPage';
import RecoveryPage from './RecoveryPage';
import { createContext, useContext, useEffect, useRef, useState } from 'react';

type AppContextType = {
  wsRef: React.RefObject<WebSocket | null>;
  uri: string;
  wsReady: boolean;
}

export const AppContext = createContext<AppContextType | null>(null);

export const useAppContext = () => {
  const context = useContext(AppContext);
  if (!context) throw new Error("useAppContext must be used inside App");
  return context;
};

function App() {
  const wsRef = useRef<WebSocket | null>(null);
  const uri = "ws://localhost:9000";
  const [wsReady, setWsReady] = useState(false);

  useEffect(() => {
        let reconnectTimeout: ReturnType<typeof setTimeout>;

        const connect = () => {
          wsRef.current = new WebSocket(uri);

          wsRef.current.onopen = () => {
                console.log("WebSocket connection established.");
                setWsReady(true);
            };

            wsRef.current.onclose = () => {
                console.log("WebSocket closed. Reconnecting in 3 seconds...");
                setWsReady(false);
                reconnectTimeout = setTimeout(connect, 3000);
            };

            wsRef.current.onerror = (error) => {
                console.error("WebSocket error:", error);
                wsRef.current?.close();
            };
        };

        connect();

        return () => {
            clearTimeout(reconnectTimeout);
            if (wsRef.current) {
              wsRef.current.onclose = null;
              wsRef.current.onerror = null;
              wsRef.current.close();
            }
        };
    }, []);
  return (
    <AppContext.Provider value={{ wsRef, uri: uri, wsReady}}>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<LandingPage />} />
          <Route path="/propulsion" element={<PropulsionPage />} />
          <Route path="/recovery" element={<RecoveryPage />} />
        </Routes>
      </BrowserRouter>
    </AppContext.Provider>

  );
}

export default App;