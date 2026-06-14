import { usePropulsion } from "../PropulsionPage";
import { useState } from "react"; 
import ConfirmationOverlay from "./ConfirmationOverlayComponent";

export default function VentButtonComponent() {

    const { ventTimeoutRef, manualVentRef, fillUIActive, ventUIActive, setVentUIActive, ventSeconds, setVentSeconds, confirmedVentSeconds, setConfirmedVentSeconds, handleButtonClickRef, isVentingRef, telemetryDataRef } = usePropulsion();
    const [showConfirmation, setShowConfirmation] = useState(false);

    // Latest tank pressure. This component re-renders on every telemetry message
    // (valve state updates), so reading the ref here stays current with the UI.
    const currentPsi = telemetryDataRef.current.at(-1)?.telemetry.pt3 ?? 0;

    // We can vent during a fill (the fill loop coordinates it), or as a standalone
    // pressure bleed-off when not filling — but only if there is pressure to release.
    const canVent = fillUIActive || currentPsi > 0;
    const toggleVentAction = () => {
        if (ventSeconds === 0) {
            alert("Please select a vent time greater than 0 seconds.");
            return;
        }
        if (ventSeconds !== confirmedVentSeconds) {
            setShowConfirmation(true);
        } else {
            alert("Vent time is already set to the selected value.");
        }
    }
    
    const handleVentConfirm = () => {
        setConfirmedVentSeconds(ventSeconds); 
        console.log(`Vent process set to last for ${ventSeconds} seconds.`); //<- inaccurate log due to state update timing, consider using ventSeconds directly in the log if needed
        setShowConfirmation(false);
      };
    
    const handleVentCancel = () => {
        setVentSeconds(ventSeconds); // Reset the dropdown to the last confirmed value
        setShowConfirmation(false);
    };
    return (
        <>
        <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-12">
            <h2 className="font-inter font-bold text-[42px] text-center mb-8">VENT BUTTON</h2>
                <div className="bg-white border-[6px] border-black rounded-2xl px-6 py-4 mb-8">
                    <label className="font-inter text-xl mb-2 block">Number of seconds to open SV2:</label>
                    <select 
                        value={ventSeconds} 
                        onChange={(e) => setVentSeconds(Number(e.target.value))}
                        className="w-full p-2 border-2 border-gray-300 rounded text-xl font-inter"
                    >
                        {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(num => (
                        <option key={num} value={num}>{num} second{num > 0 ? 's' : ''}</option>
                        ))}
                    </select>
                </div>
                <div className="flex flex-row gap-4 justify-center">
                    <button 
                    onClick={() => toggleVentAction()}
                    className="bg-[#5A87FF] border-[6px] border-black rounded-3xl px-16 py-6 font-inter font-bold text-[35px] text-white hover:opacity-90">
                        Set Time
                    </button>
                    <div className="relative group inline-block">
                        <button
                        onClick={() => {
                            if (ventUIActive) { //We Abort Here
                                //ABORT: close SV2 immediately and clear the venting lock
                                if (ventTimeoutRef.current) {
                                    clearTimeout(ventTimeoutRef.current); // cancel the scheduled completion
                                    ventTimeoutRef.current = null;
                                }
                                handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                                manualVentRef.current = false; //Set false just in case :)
                                isVentingRef.current = false;
                                setVentUIActive(false);
                            } else { //Start Manual Venting Process
                                if (isVentingRef.current) return; //Do not stack with a vent that's already running
                                if (fillUIActive) {
                                    //During a fill, hand off to the fill loop so it coordinates the vent (BV reopen, no stacking)
                                    manualVentRef.current = true;
                                } else {
                                    //Not filling: drive a standalone vent to bleed off pressure. Open SV2 now, close it after the set duration.
                                    isVentingRef.current = true;
                                    setVentUIActive(true);
                                    handleButtonClickRef.current("Solenoid Valve 2", 'OPEN');
                                    console.log("🔴 Manual Vent START (no fill):", new Date().toISOString(), "PSI:", currentPsi, `Duration: ${confirmedVentSeconds}s`);

                                    ventTimeoutRef.current = setTimeout(() => {
                                        console.log("🟢 Manual Vent END (no fill):", new Date().toISOString());
                                        handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                                        ventTimeoutRef.current = null;
                                        isVentingRef.current = false;
                                        setVentUIActive(false);
                                    }, confirmedVentSeconds * 1000);
                                }
                            }
                        }}
                        disabled={confirmedVentSeconds === 0 || !canVent}
                        className="bg-[#E05A2B] border-[4px] border-black rounded-2xl px-10 py-3 font-inter font-bold text-[32px] text-white hover:opacity-90 disabled:opacity-30 disabled:cursor-not-allowed"
                        >
                            {!ventUIActive ? "VENT" : "ABORT"}
                        </button>

                        {confirmedVentSeconds > 0 && (
                            <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1 bg-black text-white text-sm rounded-lg opacity-0 group-hover:opacity-100 transition-opacity duration-200 whitespace-nowrap pointer-events-none">
                                Vent duration: {confirmedVentSeconds} second{confirmedVentSeconds !== 1 ? 's' : ''}
                            </div>
                        )}
                    </div>
                    
                </div>
        </div>
            {showConfirmation && (
                <ConfirmationOverlay
                  message="Are You Sure"
                  onConfirm={handleVentConfirm}
                  onCancel={handleVentCancel}
                />
              )}
        </> 
    );
}