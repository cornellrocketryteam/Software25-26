import Header from "./components/HeaderComponent";
import ConfirmationOverlay from './components/ConfirmationOverlayComponent';
import { useEffect, useState } from "react";
import { useAppContext } from "./App";
import type { FlightMode } from "./App";

type BasicAction = "OPEN_PAYLOAD" | "RETRACT_PAYLOAD"; // Define basic action types for payload deployment

export function RecoveryPage() {
    const [showConfirmation, setShowConfirmation] = useState(false);
    const [pendingAction, setPendingAction] = useState< BasicAction | null>(null);
    const [payloadDeployed, setPayloadDeployed] = useState(false); // Track payload state for button logic
    const [latitude, setLatitude] = useState("");
    const [longitude, setLongitude] = useState("");
    const [confirmedCoords, setConfirmedCoords] = useState<{lat: string, lng: string} | null>(null);
    const { wsRef, wsReady, currFlightMode, setCurrFlightMode } = useAppContext();

    const toggleAction = (action: BasicAction) => {
        if (payloadDeployed && action === "OPEN_PAYLOAD" || !payloadDeployed && action === "RETRACT_PAYLOAD") { //Can update if there is a better way of getting payload state
            return; 
        } 
        setPendingAction(action);
        setShowConfirmation(true);
    };

    const handleConfirm = () => {
        if (pendingAction !== null) { //Do Something here with commands
            if(pendingAction === "OPEN_PAYLOAD"){
                setPayloadDeployed(true);
                //Send command to open payload here
            } else if (pendingAction === "RETRACT_PAYLOAD"){
                setPayloadDeployed(false);
                //Send command to retract payload here
            }
        }
        setShowConfirmation(false);
        setPendingAction(null);
      };
    
      const handleCancel = () => {
        setShowConfirmation(false);
        setPendingAction(null);
      };

    const handleMessage = (event: MessageEvent) => {
        const data = JSON.parse(event.data);
        switch(data.type) { //update fligtmode on telemetry updates
            /* Not needed as we already poll for flight mode in our initial hook in `App`
            case "fsw_telemetry": 
                setCurrFlightMode(data.flight_mode as FlightMode);
                break; 
            */
        }
    };
    
    useEffect(() => {
        let heartbeatInterval: ReturnType<typeof setInterval>; 

        if (!wsReady) return;
        

        const onOpen = () => {
            heartbeatInterval = setInterval(() => {
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                    wsRef.current.send(JSON.stringify({"command": "heartbeat"}));
                }
            }, 5000);
        }

        if (wsRef.current?.readyState === WebSocket.OPEN) {
            onOpen();
        } else {
            wsRef.current?.addEventListener('open', onOpen);
        }
    
        // Set up message listener to handle incoming messages from the server and update our state accordingly
        wsRef.current?.addEventListener('message', handleMessage);
    
        return () => {
            wsRef.current?.removeEventListener('message', handleMessage);
        };
    
    }, [wsReady]);
    
    return (
        <div className="min-h-screen bg-white">
            <Header pageTitle="Recovery & Payload Page" currFlightMode={currFlightMode}/>
        
            {/* Main Content */}
            <div className="p-8 flex flex-col gap-6">

                {/* Target Coordinates */}
                <div className="bg-[#D9D9D9] rounded-3xl px-6 py-4">
                    <h2 className="text-2xl font-inter font-semibold">
                        Target Coordinates: {confirmedCoords ? `${confirmedCoords.lat}°, ${confirmedCoords.lng}°` : ""  } {/*Show nothing until we input our chords*/}
                    </h2>
                </div>

                {/* Input Flight Info */}
                <div className="bg-[#D9D9D9] rounded-3xl p-6 flex flex-col gap-4">
                    <h3 className="text-xl font-inter font-semibold">Input Flight Info Below</h3>
                    
                    <div className="flex flex-col gap-4">
                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Latitude</label>
                            <input
                                type="number"
                                value={latitude}
                                onChange={(e) => setLatitude(e.target.value)}
                                placeholder="e.g. 42.444004375268165"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>
                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Longitude</label>
                            <input
                                type="number"
                                value={longitude}
                                onChange={(e) => setLongitude(e.target.value)}
                                placeholder="e.g. -76.48230055838474"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>
                        <button
                            onClick={() => setConfirmedCoords({ lat: latitude, lng: longitude })} //Add commands here to send the coordinates to the server and update target location
                            disabled={!latitude || !longitude} // Disable if either latitude or longitude is empty 
                            className="bg-[#5A87FF] border-[3px] border-black rounded-xl px-6 py-2 font-inter font-bold text-white text-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Confirm Coordinates
                        </button>
                    </div>
                </div>

                {/* Payload Deployment */}
                <div className="mt-auto bg-[#4A4A4A] border-[4px] border-black rounded-3xl px-6 py-4 flex items-center justify-between gap-4">
                    <span className="font-inter font-bold text-white text-xl">Payload Deployment</span>
                    <div className="flex gap-4">
                        <button onClick = {() => 
                            {
                                toggleAction("OPEN_PAYLOAD");
                                console.log("Open Payload");
                            }
                        }
                         //Add real command here to open payload
                        className="bg-[#5A87FF] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90">
                            Open
                        </button>
                        <button onClick = {() => 
                            {
                                toggleAction("RETRACT_PAYLOAD");
                                console.log("Retract Payload")
                            }
                        } //Add real command here to retract payload
                        className="bg-[#4A4A4A] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90">
                            Retract
                        </button>
                    </div>
                </div>
            </div>
            {showConfirmation && (
                <ConfirmationOverlay
                  message="Are You Sure"
                  onConfirm={handleConfirm}
                  onCancel={handleCancel}
                />
            )}
        </div>
    );
}

export default RecoveryPage;