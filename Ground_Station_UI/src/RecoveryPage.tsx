import Header from "./components/HeaderComponent";
import { useState } from "react";

export function RecoveryPage() {
    const [latitude, setLatitude] = useState("");
    const [longitude, setLongitude] = useState("");
    const [confirmedCoords, setConfirmedCoords] = useState<{lat: string, lng: string} | null>(null);

    return (
        <div className="min-h-screen bg-white">
            <Header pageTitle="Recovery & Payload Page"/>
        
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
                            onClick={() => setConfirmedCoords({ lat: latitude, lng: longitude })}
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
                        <button className="bg-[#5A87FF] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90">
                            Open
                        </button>
                        <button className="bg-[#4A4A4A] border-[3px] border-black rounded-2xl px-12 py-3 font-inter font-bold text-white text-xl hover:opacity-90">
                            Retract
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}

export default RecoveryPage;