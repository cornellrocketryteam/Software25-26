import { usePropulsion } from "../../PropulsionPage";

export default function InterveneFillComponent() {
    const { fillState, setFillState, confirmedVentSeconds, handleButtonClick, valveData} = usePropulsion();

    return (
        console.log("Rendering InterveneFillComponent with fillState: ", fillState),
        <>
            <h2 className="font-inter font-bold text-[54px] text-center mb-12">INTERVENE WITH AUTOMATED FILL</h2>
            <div className="flex flex-row items-center gap-2">
                <button 
                    onClick={() => {

                        // if(confirmedVentSeconds > 0) {
                        //     handleButtonClick("Solenoid Valve 2"); // Open SV2 to vent for the specified number of seconds
                        //     console.log(`Venting for ${confirmedVentSeconds} seconds...`);
                        //     setTimeout(() => {
                        //         handleButtonClick("Solenoid Valve 2"); // Close SV2 after venting
                        //         console.log("Venting complete. Proceeding with fill process...");
                        //     }, (confirmedVentSeconds-1) * 1000); // Convert seconds to milliseconds <-- confirmedVent duration - 1 second to account for the time it takes to send the command and for the valve to actuate, ensuring that the total venting time is as close as possible to the confirmed vent seconds.
                        //     console.log("SAFE procedure initiated. Transitioning to SAFE_PROCEDURE state.");
                        //     setFillState('SAFE_PROCEDURE');
                        // }

                        // else console.log("Please set vent seconds before initiating the safe procedure."); //should never reach this point

                        //What we do for the Safe Procedure
                        
                    }}
                    className="bg-[#2D4556] border-[6px] border-black rounded-3xl px-8 py-4 font-inter font-bold text-[48px] text-white hover:opacity-90 w-full max-w-[600px]"
                >
                    <div className="flex flex-col items-center leading-tight">
                        <span>INITIATE SAFE</span>
                        <span>PROCEDURE</span>
                    </div>
                </button>
                <button 
                    onClick={() => {
                        
                        // if(valveData.SV2.actuated) handleButtonClick("Solenoid Valve 2"); // Close SV2 if it's open
                        // if(valveData.BV.actuated) handleButtonClick("Ball Valve"); // Close BV if it's open


                        // console.log("Stop fill process initiated. Transitioning to STOP_FILL state.");
                        // setFillState('STOP_FILL');

                        //What we do for the Stop Fill Procedure
                    }}
                    className="bg-[#1A1A1A] border-[6px] border-black rounded-3xl px-8 py-2 font-inter font-bold text-[48px] text-white hover:opacity-90 w-full max-w-[600px]"
                >
                    <div className="flex flex-col items-center leading-tight">
                        <span>INITIATE</span>
                        <span>STOP FILL</span>
                    </div>
                </button>
            </div>
        </>
    );
}