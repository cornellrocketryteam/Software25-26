import { useEffect, useRef, useState } from "react";
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
        setFillState,
        handleButtonClick,
        valveData,
        telemetryDataRef,
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
        if (valveData.SV1.actuated) handleButtonClick("Solenoid Valve 1");
        if (valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2");

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

        // Open ball valve now that we know the fill loop is actually starting.
        // Only open if both SVs are confirmed closed (same guard as before).
        if (!valveData.SV1.actuated && !valveData.SV2.actuated) {
            handleButtonClick("Ball Valve");
        }

        const fillLoop = setInterval(() => {
            // Abort immediately if something externally cancelled the fill.
            const psi = telemetryDataRef.current.at(-1)?.telemetry.pt4 ?? 0;

            if(!isFillingRef.current) {clearInterval(fillLoop); return;}
            if(isVentingRef.current) return; //DO NOT STACK VENTS


            // Stop condition: target pressure reached 
            if (psi >= 975) { //<-Actual condition will change if we are running through the mock server as the CSV stops at 950
                if(valveData.BV.actuated) handleButtonClick("Ball Valve"); // Close BV
                clearInterval(fillLoop);
                isFillingRef.current = false;
                isVentingRef.current = false; // Clear any venting locks just in case
                setFillUIActive(false);
                setVentUIActive(false); // Hide vent UI if it was open
                console.log("Fill complete. Final pressure:", psi);
                return;
            }

            if(manualVentRef.current) {
                //set manual vent and isVenting
                manualVentRef.current = false; // Toggle the manual vent ref to false, we are no longer requesting a manual vent after this
                isVentingRef.current = true; //We are in fact venting now, so set this to true to prevent the auto vent logic from trying to run at the same time
                setVentUIActive(isVentingRef.current);
                console.log("🔴 Vent START:", new Date().toISOString(), "PSI:", psi, "Vent duration:", confirmedVentSeconds * 1000, "ms");


                if(valveData.BV.actuated)   handleButtonClick("Ball Valve");        // Close BV before venting
                if(!valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2");  // Open SV2 to begin vent
                //Set the time out for how long we want it
                setTimeout(() => {
                    console.log("🟢 Vent END:", new Date().toISOString());
                    if(valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2");  // Close SV2 after 1s
                    if(!valveData.BV.actuated) handleButtonClick("Ball Valve");        // Reopen BV to resume fill

                    // Clear the venting lock so the interval can resume normal checks.
                    isVentingRef.current = false;
                    setVentUIActive(isVentingRef.current);
                }, confirmedVentSeconds * 1000); // confirmed vent duration
                return;
            }

            // Vent condition: crossed a 100PSI threshold 
            if (psi >= thresholdRef.current) {
                // Mark venting so subsequent ticks don't re-enter this block.
                isVentingRef.current = true;
                setVentUIActive(isVentingRef.current);
                if(valveData.BV.actuated)   handleButtonClick("Ball Valve");        // Close BV before venting
                if(!valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2");  // Open SV2 to begin vent

                // Raise the threshold now (via ref) so it's immediately visible
                // to the next tick — no stale state issue.
                thresholdRef.current += 100;

                // Also sync React state for any dependent UI.
                setThresholdPressure(thresholdRef.current);

                setTimeout(() => {
                    if(valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2");  // Close SV2 after 1s
                    if(!valveData.BV.actuated) handleButtonClick("Ball Valve");        // Reopen BV to resume fill

                    // Clear the venting lock so the interval can resume normal checks.

                    isVentingRef.current = false;
                    setVentUIActive(isVentingRef.current);
                }, 1000); // 1-second vent duration
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
                        }}
                        className="bg-[#2D4556] border-[6px] border-black rounded-3xl px-8 py-2 font-inter font-bold text-[36px] text-white hover:opacity-90 flex-1"
                    >
                        <div className="flex flex-col items-center leading-tight">
                            <span>INITIATE SAFE</span>
                            <span>PROCEDURE</span>
                        </div>
                    </button>
                    <button 
                        onClick={() => {  setFillState("STOP_FILL");
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