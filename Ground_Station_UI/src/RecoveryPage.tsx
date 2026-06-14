import Header from "./components/HeaderComponent";
import ConfirmationOverlay from './components/ConfirmationOverlayComponent';
import TargetMap from './components/TargetMapComponent';
import { useEffect, useState } from "react";
import { useAppContext } from "./App";

type BasicAction = "OPEN_PAYLOAD";

export function RecoveryPage() {
    const [showConfirmation, setShowConfirmation] = useState(false);
    const [pendingAction, setPendingAction] = useState< BasicAction | null>(null);
    const [latitudeTar1, setLatitudeTar1] = useState("");
    const [longitudeTar1, setLongitudeTar1] = useState("");
    const [latitudeTar2, setLatitudeTar2] = useState("");
    const [longitudeTar2, setLongitudeTar2] = useState("");
    const [confirmedCoords, setConfirmedCoords] = useState<{lat_1: string, lng_1: string, lat_2: string, lng_2: string} | null>(null);
    const { wsRef, wsReady, currFlightMode } = useAppContext();

    const toggleAction = (action: BasicAction) => {
        setPendingAction(action);
        setShowConfirmation(true);
    };

    const handleConfirm = () => {
        if (pendingAction !== null) {
            if(pendingAction === "OPEN_PAYLOAD"){
                wsRef.current?.send(JSON.stringify({"command": "fsw_payload_n1"})); //Send command to extend payload, IMMEDIATELY, no checks needed.
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
    
    const basicValidCheckForCoords = (lat: number, lng: number) => {
        if (isNaN(lat) || isNaN(lng)) {
            alert("Please enter valid numeric values for latitude and longitude.");
            return false;
        }
        if (lat < -90 || lat > 90) {
            alert("Latitude must be between -90 and 90 degrees.");
            return false;
        }
        if (lng < -180 || lng > 180) {
            alert("Longitude must be between -180 and 180 degrees.");
            return false;
        }
        return true;
    }

    useEffect(() => {
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
                <div className="bg-[#D9D9D9] flex flex-col rounded-3xl px-6 py-4 gap-3">
                    <div>
                        <h2 className="text-2xl font-inter font-semibold">
                            Target Coordinates:
                        </h2>
                    </div>
                    {confirmedCoords && (
                        <div className="flex flex-col gap-4">
                            <h2 className="text-2xl font-inter flex gap-4">
                                <span className="font-bold">Upwind Target:</span>
                                <span>{`Lat: ${confirmedCoords.lat_1}°`}</span>
                                <span>{`Lon: ${confirmedCoords.lng_1}°`}</span>
                            </h2>
                            <h2 className="text-2xl font-inter flex gap-4">
                                <span className="font-bold">Downwind Target:</span>
                                <span>{`Lat: ${confirmedCoords.lat_2}°`}</span>
                                <span>{`Lon: ${confirmedCoords.lng_2}°`}</span>
                            </h2>
                        </div>
                    )}
                </div>

                {/* Target Map visualizer */}
                <div className="bg-[#D9D9D9] flex flex-col rounded-3xl px-6 py-4 gap-3">
                    <h2 className="text-2xl font-inter font-semibold">
                        Target Map:
                    </h2>
                    <TargetMap coords={confirmedCoords} />
                </div>

                {/* Input Flight Info */}
                <div className="bg-[#D9D9D9] rounded-3xl p-6 flex flex-col gap-4">
                    <h3 className="text-xl font-inter font-semibold">Input Flight Info Below</h3>

                    <div className="flex flex-col gap-4">
                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Upwind Latitude</label>
                            <input
                                type="number"
                                value={latitudeTar1}
                                onChange={(e) => setLatitudeTar1(e.target.value)}
                                placeholder="e.g. 42.444004375268165"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>
                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Upwind Longitude</label>
                            <input
                                type="number"
                                value={longitudeTar1}
                                onChange={(e) => setLongitudeTar1(e.target.value)}
                                placeholder="e.g. -76.48230055838474"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>

                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Downwind Latitude</label>
                            <input
                                type="number"
                                value={latitudeTar2}
                                onChange={(e) => setLatitudeTar2(e.target.value)}
                                placeholder="e.g. 42.444004375268165"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>
                        <div className="flex flex-col gap-1">
                            <label className="font-inter text-lg">Downwind Longitude</label>
                            <input
                                type="number"
                                value={longitudeTar2}
                                onChange={(e) => setLongitudeTar2(e.target.value)}
                                placeholder="e.g. -76.48230055838474"
                                className="border-[3px] border-black rounded-xl px-4 py-2 font-inter text-lg bg-white focus:outline-none"
                            />
                        </div>
                        <button
                            onClick={() => {
                                if(!basicValidCheckForCoords(Number(latitudeTar1), Number(longitudeTar1)) || !basicValidCheckForCoords(Number(latitudeTar2), Number(longitudeTar2))) return;
                                const lat_tar1 = Number(latitudeTar1);
                                const lon_tar1 = Number(longitudeTar1);
                                const lat_tar2 = Number(latitudeTar2);
                                const lon_tar2 = Number(longitudeTar2);
                                setConfirmedCoords({lat_1: latitudeTar1, lng_1: longitudeTar1, lat_2: latitudeTar2, lng_2: longitudeTar2});
                            
                                wsRef.current?.send(JSON.stringify({ "command": "fsw_set_blims_target", lat_tar1, lon_tar1, lat_tar2, lon_tar2 }));
                            }}
                            disabled={!latitudeTar1 || !longitudeTar1 || !latitudeTar2 || !longitudeTar2}
                            className="bg-[#5A87FF] border-[3px] border-black rounded-xl px-6 py-2 font-inter font-bold text-white text-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Confirm Coordinates
                        </button>
                    </div>
                </div>

                {/* Payload Deployment */}
                <div className="mt-auto bg-[#4A4A4A] border-[4px] border-black rounded-3xl px-6 py-4 flex items-center justify-between gap-4">
                    <span className="font-inter font-bold text-white text-xl">
                    Payload Deployment
                    </span>
                    <div className="flex gap-4">
                        <button
                            onClick={() => toggleAction("OPEN_PAYLOAD")}
                            className="bg-[#5A87FF] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Open
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
