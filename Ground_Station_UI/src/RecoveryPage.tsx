import Header from "./components/HeaderComponent";
import { useEffect, useState } from "react";
import { useAppContext } from "./App";
import ConfirmationOverlay from "./components/ConfirmationOverlayComponent";

export function RecoveryPage() {
    const [latitude, setLatitude] = useState("");
    const [longitude, setLongitude] = useState("");
    const [confirmedCoords, setConfirmedCoords] = useState<{lat: string, lng: string} | null>(null);
    const [pendingPayload, setPendingPayload] = useState<string | null>(null);
    const { wsRef, wsReady, currFlightMode } = useAppContext();

    const handleMessage = (event: MessageEvent) => {
        const data = JSON.parse(event.data);
        switch(data.type) {
            // Recovery specific message handling
        }
    };

    useEffect(() => {
        if (!wsReady) return;
        wsRef.current?.addEventListener('message', handleMessage);
        return () => {
            wsRef.current?.removeEventListener('message', handleMessage);
        };
    }, [wsReady]);

    const sendBlimsTarget = () => {
        const lat = parseFloat(latitude);
        const lon = parseFloat(longitude);
        if (isNaN(lat) || isNaN(lon)) return;
        wsRef.current?.send(JSON.stringify({ command: "fsw_set_blims_target", lat, lon }));
        setConfirmedCoords({ lat: latitude, lng: longitude });
    };

    const sendPayloadCommand = (command: string) => {
        wsRef.current?.send(JSON.stringify({ command }));
        setPendingPayload(null);
    };

    return (
        <div className="min-h-screen bg-white">
            <Header pageTitle="Recovery & Payload Page" currFlightMode={currFlightMode}/>

            {/* Main Content */}
            <div className="p-8 flex flex-col gap-6">

                {/* Target Coordinates */}
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
                            onClick={sendBlimsTarget}
                            disabled={!latitude || !longitude}
                            className="bg-[#5A87FF] border-[3px] border-black rounded-xl px-6 py-2 font-inter font-bold text-white text-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Confirm Coordinates
                        </button>
                    </div>
                </div>

                {/* Payload Deployment */}
                <div className="bg-[#4A4A4A] border-[4px] border-black rounded-3xl px-6 py-4 flex flex-col gap-4">
                    <span className="font-inter font-bold text-white text-xl">Payload Deployment</span>
                    <div className="grid grid-cols-2 gap-4">
                        {[
                            { label: "N1", command: "fsw_payload_n1" },
                            { label: "N2", command: "fsw_payload_n2" },
                            { label: "N3", command: "fsw_payload_n3" },
                            { label: "N4", command: "fsw_payload_n4" },
                        ].map(({ label, command }) => (
                            <button
                                key={command}
                                onClick={() => setPendingPayload(command)}
                                className="bg-[#5A87FF] border-[3px] border-black rounded-2xl px-8 py-3 font-inter font-bold text-white text-xl hover:opacity-90"
                            >
                                {label}
                            </button>
                        ))}
                    </div>
                </div>
            </div>

            {pendingPayload && (
                <ConfirmationOverlay
                    message={`Fire ${pendingPayload.toUpperCase()}?`}
                    onConfirm={() => sendPayloadCommand(pendingPayload)}
                    onCancel={() => setPendingPayload(null)}
                />
            )}
        </div>
    );
}

export default RecoveryPage;
