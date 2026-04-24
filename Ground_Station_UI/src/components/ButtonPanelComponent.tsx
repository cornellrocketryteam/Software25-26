import { useState } from "react";
import ButtonComponent from "./ButtonComponent";
import { usePropulsion } from "../PropulsionPage";



export default function ButtonPanelComponent() {
    const [isExpanded, setIsExpanded] = useState(false);
    const {fillState, telemetryDataRef } = usePropulsion();
    

    const renderContent  = () => {   
        if(!isExpanded) {
            return(
                <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-8 flex items-center justify-between">
                    <div>
                        <p className="font-inter text-2xl font-bold">Expand Button Panel:</p>
                        <p className="font-inter text-lg">(Will display launch button)</p>
                    </div>
                    <button 
                        onClick = {() => 
                            {
                                if(fillState !== 'INITIAL' || (telemetryDataRef.current.at(-1)?.telemetry.pt4 ?? 0) >= 975){ // Allow expanding if we are not in the initial fill state or if we have reached at least 750 psi, which is close enough to max fill to allow launch preparations{   
                                    setIsExpanded(true);
                                }
                            }
                        }
                        className={fillState !== 'INITIAL' || (telemetryDataRef.current.at(-1)?.telemetry.pt4 ?? 0) >= 975 ? "bg-[#4F4B40] border-[6px] border-black rounded-full w-24 h-24 flex items-center justify-center hover:opacity-90" :
                            "bg-[#4F4B40]/50 border-[6px] border-black rounded-full w-24 h-24 flex items-center justify-center cursor-not-allowed opacity-50"
                         }>
                        <svg className="w-12 h-12" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3">
                            <path d="M12 5v14M5 12h14" />
                        </svg>
                    </button>
                </div>
            );
        }

        return(
            <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-8 flex items-center justify-between">
                <div>
                    <p className="font-inter text-2xl font-bold">Shrink Button Panel:</p>
                    <p className="font-inter text-lg">(Will hide launch button)</p>
                </div>

                <ButtonComponent buttonName = {"Launch Button"} currentState={false} actuationLock='LOCKED' showState = {false}/>

                <button 
                    onClick = {() => setIsExpanded(false)}
                    className="bg-[#4F4B40] border-[6px] border-black rounded-full w-24 h-24 flex items-center justify-center hover:opacity-90">
                    <svg className="w-12 h-12" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3">
                        <path d="M5 12h14" />
                    </svg>
                </button>
            </div>
        );
    };

    return (
        <div>
            {renderContent()}
        </div>
    );
}