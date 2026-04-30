import Header from "./components/HeaderComponent";
import ConfirmationOverlay from './components/ConfirmationOverlayComponent';
import { useEffect, useState } from "react";
import { useAppContext } from "./App";

type BasicAction = "OPEN_PAYLOAD" | "RETRACT_PAYLOAD";

export function RecoveryPage() {
    const [showConfirmation, setShowConfirmation] = useState(false);
    const [pendingAction, setPendingAction] = useState< BasicAction | null>(null);
    const [latitude, setLatitude] = useState("");
    const [longitude, setLongitude] = useState("");
    const [confirmedCoords, setConfirmedCoords] = useState<{lat: string, lng: string} | null>(null);
    const { wsRef, wsReady, currFlightMode } = useAppContext();

    const toggleAction = (action: BasicAction) => {
        if (payloadDeployed && action === "OPEN_PAYLOAD" || !payloadDeployed && action === "RETRACT_PAYLOAD") {
            return;
        }
        setPendingAction(action);
        setShowConfirmation(true);
    };

    const handleConfirm = () => {
        if (pendingAction !== null) { //Do Something here with commands
            if(pendingAction === "OPEN_PAYLOAD"){
                wsRef.current?.send(JSON.stringify(extendCommand)); //Send command to extend payload
            }
        }
        setShowConfirmation(false);
        setPendingAction(null);
    };

    const handleCancel = () => {
        setShowConfirmation(false);
        setPendingAction(null);
    };

    const handleMessage = (_event: MessageEvent) => {
        // Reserved for future telemetry handling on this page
    };

    useEffect(() => {
        // Bug 9 fix: declare outside onOpen so cleanup can clear it
        let heartbeatInterval: ReturnType<typeof setInterval>;

        if (!wsReady) return;

        const onOpen = () => {
            heartbeatInterval = setInterval(() => {
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                    wsRef.current.send(JSON.stringify({ "command": "heartbeat" }));
                }
            }, 5000);
        };

        if (wsRef.current?.readyState === WebSocket.OPEN) {
            onOpen();
        } else {
            wsRef.current?.addEventListener('open', onOpen);
        }

        wsRef.current?.addEventListener('message', handleMessage);

        return () => {
            clearInterval(heartbeatInterval);
            wsRef.current?.removeEventListener('open', onOpen);
            wsRef.current?.removeEventListener('message', handleMessage);
        };

    }, [wsReady]);

    return (
        <div className="min-h-screen bg-white">
            <Header pageTitle="Recovery & Payload Page" currFlightMode={currFlightMode}/>

            {/* Main Content */}
            <div className="p-8 flex flex-col gap-6">

                {/* Target Coordinates display */}
                <div className="bg-[#D9D9D9] rounded-3xl px-6 py-4">
                    <h2 className="text-2xl font-inter font-semibold">
                        Target Coordinates: {confirmedCoords ? `${confirmedCoords.lat}°, ${confirmedCoords.lng}°` : ""}
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
                            onClick={() => {
                                const lat = Number(latitude);
                                const lon = Number(longitude);
                                setConfirmedCoords({ lat: latitude, lng: longitude });
                                // Bug 4 fix: actually send the command with correct field names (lat/lon per server)
                                wsRef.current?.send(JSON.stringify({ "command": "fsw_set_blims_target", lat, lon }));
                            }}
                            disabled={!latitude || !longitude}
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
                        <button
                            onClick={() => toggleAction("OPEN_PAYLOAD")}
                            disabled={payloadDeployed}
                            className="bg-[#5A87FF] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Open
                        </button>
                        <button
                            onClick={() => toggleAction("RETRACT_PAYLOAD")}
                            disabled={!payloadDeployed}
                            className="bg-[#4A4A4A] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
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
