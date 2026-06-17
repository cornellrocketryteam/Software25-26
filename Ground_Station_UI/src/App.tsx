import { BrowserRouter, Routes, Route } from 'react-router-dom';
import LandingPage from './LandingPage';
import PropulsionPage from './PropulsionPage';
import RecoveryPage from './RecoveryPage';
import { createContext, useContext, useEffect, useRef, useState } from 'react';

type AppContextType = {
  wsRef: React.RefObject<WebSocket | null>;
  currFlightMode: FlightMode;
  setCurrFlightMode: (flightMode: FlightMode) => void;
  uri: string;
  wsReady: boolean;
  hasLaunched: boolean;
  setHasLaunched: (launched: boolean) => void;
}
export type FlightMode = "....."| 'Standby' | 'Startup' | 'Ascent' | 'Coast' | 'DrogueDeployed' | 'MainDeployed' | 'Fault';

export const AppContext = createContext<AppContextType | null>(null);

export const useAppContext = () => {
  const context = useContext(AppContext);
  if (!context) throw new Error("useAppContext must be used inside App");
  return context;
};



function App() {
  const wsRef = useRef<WebSocket | null>(null);

  const uri = "ws://192.168.1.106:9000"; //This is the actual router IP we would use for launch, so make sure any pushed code has this line commented out
  //const uri = "ws://localhost:9000"; //This is for testing on the mock server
  const [wsReady, setWsReady] = useState(false);
  const [currFlightMode, setCurrFlightMode] = useState<FlightMode>('.....');

  // Track whether we've launched. Backed by sessionStorage so the locked-out launch
  // state survives a page refresh, but auto-clears when the tab closes (a new session
  // starts unlocked, rather than latching forever as localStorage would).
  const [hasLaunched, setHasLaunched] = useState(() => sessionStorage.getItem('hasLaunched') === 'true');

  const handleMessage = (event: MessageEvent) => {
    //Parse JSON data here
    const data = JSON.parse(event.data);
    if(data.type === "fsw_telemetry"){
      setCurrFlightMode(data.flight_mode as FlightMode);
    }
  }
  
  useEffect(() => {
        let reconnectTimeout: ReturnType<typeof setTimeout>;
        let heartbeatInterval: ReturnType<typeof setInterval>;

        const connect = () => {
          wsRef.current = new WebSocket(uri);

          wsRef.current.onopen = () => {
                console.log("WebSocket connection established.");
                setWsReady(true);
                // Global heartbeat — keeps the connection alive on every page.
                // Server disconnects after 15s without one (main.rs:231).
                heartbeatInterval = setInterval(() => {
                    if (wsRef.current?.readyState === WebSocket.OPEN) {
                        wsRef.current.send(JSON.stringify({ "command": "heartbeat" }));
                        console.log("Sent Heartbeat");
                    }
                }, 5000);
            };

            wsRef.current.onclose = () => {
                console.log("WebSocket closed. Reconnecting in 3 seconds...");
                clearInterval(heartbeatInterval);
                setWsReady(false);
                reconnectTimeout = setTimeout(connect, 3000);
            };

            wsRef.current.onerror = (error) => {
                console.error("WebSocket error:", error);
                wsRef.current?.close();
            };
            wsRef.current.addEventListener('message', handleMessage);
        };

        connect();

        return () => {
            clearInterval(heartbeatInterval);
            clearTimeout(reconnectTimeout);
            if (wsRef.current) {
              wsRef.current.removeEventListener('message', handleMessage);
              wsRef.current.onclose = null;
              wsRef.current.onerror = null;
              wsRef.current.close();
            }
        };
    }, []);
  return (
    <AppContext.Provider value={{hasLaunched, setHasLaunched, setCurrFlightMode, currFlightMode, wsRef, uri: uri, wsReady}}>
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