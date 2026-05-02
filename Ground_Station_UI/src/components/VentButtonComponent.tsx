import { usePropulsion } from "../PropulsionPage";
import { useState } from "react"; 
import ConfirmationOverlay from "./ConfirmationOverlayComponent";

export default function VentButtonComponent() {

    const { manualVentRef, fillUIActive, ventUIActive, setVentUIActive, ventSeconds, setVentSeconds, confirmedVentSeconds, setConfirmedVentSeconds, handleButtonClickRef, isVentingRef } = usePropulsion();
    const [showConfirmation, setShowConfirmation] = useState(false);
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
    //If we say yes, then we will actually be doin it :)
    const handleVentConfirm = () => {
        setConfirmedVentSeconds(ventSeconds); 
        console.log(`Vent process set to last for ${ventSeconds} seconds.`); //<- inaccurate log due to state update timing, consider using ventSeconds directly in the log if needed
        setShowConfirmation(false);
      };
    
      //We will not be doin it chat
    const handleVentCancel = () => {
        setVentSeconds(0);
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
                            if (ventUIActive) {
                                // ABORT: close SV2 immediately and clear the venting lock
                                handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                                isVentingRef.current = false;
                                setVentUIActive(false);
                            } else {
                                manualVentRef.current = true;
                            }
                        }}
                        disabled={confirmedVentSeconds === 0 || !fillUIActive}
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