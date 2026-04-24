import { useEffect, useRef } from "react";
import { usePropulsion } from "../../PropulsionPage";

export default function InitialFillComponent() {
    const {
        manualVentRef,
        fillUIActive,
        ventUIActive,
        setFillUIActive,
        setVentUIActive,
        isFillingRef,
        isVentingRef,
        confirmedVentSeconds,
        thresholdPressure,
        setThresholdPressure,
        fillState,
        confirmedVentSecondsRef,
        handleButtonClickRef,
        valveDataRef,
        telemetryDataRef,
        canInteractRef,
        setButtonInteractionState
    } = usePropulsion();

    console.log("Rendering InitialFillComponent with fillState: ", fillState);

    //Fill state driven by a boolean so the loop lives in useEffect

    // useRef for the threshold so the interval always reads the latest
    // value without needing to be recreated every time React state updates.
    const thresholdRef = useRef(thresholdPressure);
    
    //  Click handler: only responsible for pre-checks and setting the  
    //  fillUIActive flag. The actual loop is managed in the useEffect below.
    const handleInitiate = () => {
        // Safety: close any open valves before starting to ensure a known state.
        if (valveDataRef.current.SV1.actuated) handleButtonClickRef.current("Solenoid Valve 1", 'CLOSE');
        if (valveDataRef.current.SV2.actuated) handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');

        // Snapshot the starting pressure for reference (available for future use).
        const startingPressure = telemetryDataRef.current.at(-1)?.telemetry.pt4 ?? 0;
        console.log("Fill initiated. Starting pressure:", startingPressure);

        // Reset threshold ref to current thresholdPressure before each new fill run.
        thresholdRef.current = thresholdPressure;

        // Flip the fill flag — this triggers the useEffect loop below.
        isFillingRef.current = true;
        setFillUIActive(true);
    };

    //Fill loop: runs whenever fillUIActive becomes true.               
    //Cleaned up automatically when fillUIActive is set to false or the component unmounts.                                             
    useEffect(() => {
        if (!fillUIActive) return;

        if (canInteractRef.current === 'ENABLED') {
            setButtonInteractionState('DISABLED');
            canInteractRef.current = 'DISABLED';
        }

        // Open ball valve now that we know the fill loop is actually starting.
        // Only open if both SVs are confirmed closed (same guard as before).

        //Close both SVs, then call open BV.
        if (valveDataRef.current.SV1.actuated) {
            handleButtonClickRef.current("Solenoid Valve 1", 'CLOSE');
        }
        if (valveDataRef.current.SV2.actuated) {
            handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
        }

        // Delay BV open to give SV close commands + server response time to settle
        setTimeout(() => {
            if (!valveDataRef.current.SV1.actuated && !valveDataRef.current.SV2.actuated) {
                handleButtonClickRef.current("Ball Valve", 'OPEN');
            }
        }, 600); // buttondelay (250) + query delay (50) + server response time + buffer

        const fillLoop = setInterval(() => {
            // Abort immediately if something externally cancelled the fill.
            const psi = telemetryDataRef.current.at(-1)?.telemetry.pt4 ?? 0;

            if(!isFillingRef.current) {clearInterval(fillLoop); return;}
            if(isVentingRef.current) return; //DO NOT STACK VENTS


            // Stop condition: target pressure reached 
            if (psi >= 975) { //<-Actual condition will change if we are running through the mock server as the CSV stops at 950
                if(valveDataRef.current.BV.actuated) handleButtonClickRef.current("Ball Valve", 'CLOSE'); // Close BV
                clearInterval(fillLoop);
                isFillingRef.current = false;
                isVentingRef.current = false; // Clear any venting locks just in case
                setFillUIActive(false);
                setVentUIActive(false); // Hide vent UI if it was open
                console.log("Fill complete. Final pressure:", psi);
                return;
            }

            // Vent condition: crossed a 100PSI threshold 
            // Manual Vent has precedence over auto vent, so we check that first and return early if it's set. This prevents the scenario where an auto vent triggers on the same tick as a manual vent request, which could cause conflicts in valve commands and vent duration.
            if (psi >= thresholdRef.current || manualVentRef.current) {
                // Mark venting so subsequent ticks don't re-enter this block.
                const isManualVent = manualVentRef.current; // Capture whether this vent was manual for logging and timeout purposes
                manualVentRef.current = false; // Reset the manual vent request immediately to prevent re-entry on the next tick
                isVentingRef.current = true;
                setVentUIActive(isVentingRef.current);
                handleButtonClickRef.current("Ball Valve", 'CLOSE');        // Close BV before venting
                handleButtonClickRef.current("Solenoid Valve 2", 'OPEN');  // Open SV2 to begin vent
                console.log(`🔴 ${isManualVent ? "Manual" : "Auto"} Vent START:`, new Date().toISOString(), "PSI:", psi, `Vent duration: ${isManualVent ? confirmedVentSeconds * 1000 + " ms" : "1000 ms (auto vent)"}`);

                // Raise the threshold (via ref) so it's immediately visible
                // to the next tick only when our current vent isn't manual.
                if(!isManualVent) {
                    thresholdRef.current += 100;
                    // Also sync React state for any dependent UI.
                    setThresholdPressure(thresholdRef.current);
                }

                setTimeout(() => {
                    console.log("🟢 Vent END:", new Date().toISOString());
                    handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');  // Close SV2 after 1s
                    handleButtonClickRef.current("Ball Valve", 'OPEN');        // Reopen BV to resume fill

                    // Clear the venting lock so the interval can resume normal checks.
                    isVentingRef.current = false;
                    setVentUIActive(isVentingRef.current);
                }, (isManualVent ? confirmedVentSecondsRef.current * 1000 : 1000)); // 1-second vent duration
            }
        }, 200); // Poll telemetry every 200ms <-Rough value, might change

        // Cleanup: clear the interval if fillUIActive is set to false or
        // the component unmounts mid-fill.
        return () => {
            clearInterval(fillLoop);
        };

    // Re-run only when fillUIActive changes — valveData intentionally omitted
    // from deps to avoid restarting the loop on every telemetry update.
    }, [fillUIActive]);

    return (
        <>
            <h2 className="font-inter font-bold text-[60px] text-center mb-12">
                {fillUIActive ? "FILL IN PROGRESS": "INITIATE AUTOMATED FILL"}
            </h2>
            <div className="flex flex-col gap-4 justify-center">
                <button
                    onClick={handleInitiate}
                    // Disable the button while a fill is already in progress
                    // to prevent double-initiating the loop.
                    disabled={fillUIActive}
                    className="bg-[#5A87FF] border-[6px] border-black rounded-3xl px-24 py-8 font-inter font-bold text-[69px] text-white disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    {fillUIActive ? (ventUIActive ? "Venting..." : "Filling...") : "Initiate"}
                </button>
                {fillUIActive && <div className="flex flex-row items-center gap-2">
                    <button 
                        onClick={() => {
                            handleButtonClickRef.current("Ball Valve", 'CLOSE'); // Close BV to stop fill
                            isVentingRef.current = true; // Clear any venting locks just in case
                            ventUIActive && setVentUIActive(false); // Hide vent UI if it was open
                        }}
                        className="bg-[#2D4556] border-[6px] border-black rounded-3xl px-8 py-2 font-inter font-bold text-[36px] text-white hover:opacity-90 flex-1"
                    >
                        <div className="flex flex-col items-center leading-tight">
                            <span>INITIATE SAFE</span>
                            <span>PROCEDURE</span>
                        </div>
                    </button>
                    <button 
                        onClick={() => {  //close BV1. Do NOT vent SV2, leave both SVs closed and BV closed
                            handleButtonClickRef.current("Ball Valve", 'CLOSE');
                            isVentingRef.current = false; // Clear any venting locks just in case
                            setFillUIActive(false);
                        }}
                        className="bg-[#1A1A1A] border-[6px] border-black rounded-3xl px-8 py-2 font-inter font-bold text-[36px] text-white hover:opacity-90 flex-1"
                    >
                        <div className="flex flex-col items-center leading-tight">
                            <span>INITIATE</span>
                            <span>STOP FILL</span>
                        </div>
                    </button>
                </div>}
            </div>
        </>
    );
}