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
        fillState,
        setFillState,
        confirmedVentSecondsRef,
        handleButtonClickRef,
        valveDataRef,
        telemetryDataRef,
        canInteractRef,
        setButtonInteractionState
    } = usePropulsion();

    console.log("Rendering InitialFillComponent with fillState: ", fillState);


    //Checks to see if we already hit the vent threshold
    const hasAutoVentedRef = useRef(false);

    
    //  Click handler: only responsible for pre-checks and setting the  
    //  fillUIActive flag. The actual loop is managed in the useEffect below.
    const handleInitiate = () => {
        // Safety: close any open valves before starting to ensure a known state.
        if (valveDataRef.current.SV1.actuated) handleButtonClickRef.current("Solenoid Valve 1", 'CLOSE');
        if (valveDataRef.current.SV2.actuated) handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');

        const startingPressure = telemetryDataRef.current.at(-1)?.telemetry.pt3 ?? 0;
        console.log("Fill initiated. Starting pressure:", startingPressure);

        // Reset one-time vent flags and locks in case of re-initiation without page refresh.
        hasAutoVentedRef.current = false;

        isFillingRef.current = true;
        setFillUIActive(true);
    };

    useEffect(() => {
        if (!fillUIActive) return;

        if (canInteractRef.current === 'ENABLED') {
            setButtonInteractionState('DISABLED');
            canInteractRef.current = 'DISABLED';
        }

        if (valveDataRef.current.SV1.actuated) handleButtonClickRef.current("Solenoid Valve 1", 'CLOSE');
        if (valveDataRef.current.SV2.actuated) handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');

        // Delay BV open to give SV close commands + server response time to settle
        setTimeout(() => {
            if (!valveDataRef.current.SV1.actuated && !valveDataRef.current.SV2.actuated) {
                handleButtonClickRef.current("Ball Valve", 'OPEN');
            }
        }, 600);

        const fillLoop = setInterval(() => {
            const psi = telemetryDataRef.current.at(-1)?.telemetry.pt3 ?? 0;

            if (!isFillingRef.current) { clearInterval(fillLoop); return; }
            if (isVentingRef.current) return; // DO NOT STACK VENTS

            // Stop condition: target pressure reached 
            if (psi >= 900) { //<-Actual condition will change if we are running through the mock server as the CSV stops at 950
                if(valveDataRef.current.BV.actuated) handleButtonClickRef.current("Ball Valve", 'CLOSE'); // Close BV
                clearInterval(fillLoop);
                isFillingRef.current = false;
                isVentingRef.current = false;
                setFillUIActive(false);
                setVentUIActive(false);
                console.log("Fill complete. Final pressure:", psi);
                return;
            }

            // Auto-vent condition: one single 1-second vent once PT3 hits 800 PSI
            if (psi >= 800 && !hasAutoVentedRef.current) {
                hasAutoVentedRef.current = true; // Prevent re-triggering
            // Auto-vent condition: one single 1-second vent once PT3 hits 800 PSI
            if (psi >= 800 && !hasAutoVentedRef.current) {
                hasAutoVentedRef.current = true; // Prevent re-triggering
                isVentingRef.current = true;
                setVentUIActive(true);
                //handleButtonClickRef.current("Ball Valve", 'CLOSE');
                handleButtonClickRef.current("Solenoid Valve 2", 'OPEN');
                console.log("🔴 Auto Vent START:", new Date().toISOString(), "PSI:", psi);

                setTimeout(() => {
                    console.log("🟢 Auto Vent END:", new Date().toISOString());
                    handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                    handleButtonClickRef.current("Ball Valve", 'OPEN');
                    isVentingRef.current = false;
                    setVentUIActive(false);
                }, 1000);
                return;
            }

            // Manual vent: operator-triggered via the Vent Button
            if (manualVentRef.current) {
                manualVentRef.current = false;
                isVentingRef.current = true;
                setVentUIActive(true);
                //handleButtonClickRef.current("Ball Valve", 'CLOSE');
                handleButtonClickRef.current("Solenoid Valve 2", 'OPEN');
                console.log("🔴 Manual Vent START:", new Date().toISOString(), "PSI:", psi, `Duration: ${confirmedVentSecondsRef.current}s`);

                setTimeout(() => {
                    console.log("🟢 Manual Vent END:", new Date().toISOString());
                    handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                    handleButtonClickRef.current("Ball Valve", 'OPEN');
                    console.log("🟢 Manual Vent END:", new Date().toISOString());
                    handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                    handleButtonClickRef.current("Ball Valve", 'OPEN');
                    isVentingRef.current = false;
                    setVentUIActive(false);
                }, confirmedVentSecondsRef.current * 1000);
                    setVentUIActive(false);
                }, confirmedVentSecondsRef.current * 1000);
            }
        }, 200);

        return () => { clearInterval(fillLoop); };

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
                            // Stop the fill loop
                            isFillingRef.current = false;
                            setFillUIActive(false);
                            // Close BV, then open SV2 to vent indefinitely
                            // Operator must manually close SV2 via the valve grid when ready
                            handleButtonClickRef.current("Ball Valve", 'CLOSE');
                            handleButtonClickRef.current("Solenoid Valve 2", 'OPEN');
                            isVentingRef.current = false;
                            setVentUIActive(false);
                            // Re-enable button interaction so operator can close SV2
                            setButtonInteractionState('ENABLED');
                            canInteractRef.current = 'ENABLED';
                            //setFillState('SAFE_PROCEDURE');
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
                            // Close SV2 first if a vent is mid-cycle
                            if (isVentingRef.current) handleButtonClickRef.current("Solenoid Valve 2", 'CLOSE');
                            handleButtonClickRef.current("Ball Valve", 'CLOSE');
                            isFillingRef.current = false;
                            isVentingRef.current = false;
                            isFillingRef.current = false;
                            isVentingRef.current = false;
                            setFillUIActive(false);
                            setVentUIActive(false);
                            setVentUIActive(false);
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