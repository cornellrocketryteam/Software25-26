import InitialFillComponent from "./subcomponents/InitialFillComponent";
import InterveneFillComponent from "./subcomponents/InterveneFillComponent";
import SafeFillComponent from "./subcomponents/SafeFillComponent";
import StopFillComponent from "./subcomponents/StopFillComponent";
import { usePropulsion } from "../PropulsionPage";


export default function FillButtonComponent() {
    const { fillState } = usePropulsion();
    
    const renderContnet  = () => {
        switch (fillState) {
            case 'INITIAL':
                return (
                    <InitialFillComponent />
                );

            
            // case 'INTERVENE':
            //     return (
            //         <InterveneFillComponent />
            //     );

            
            case 'SAFE_PROCEDURE':
                return(
                    <SafeFillComponent/>
                );  

            case 'STOP_FILL': 
                return (
                    <StopFillComponent/>
                );

            //Add POST_FILL case here

            default:
                return (
                    <>
                        <h2 className="font-inter font-bold text-[48px] text-center mb-12">START AUTOMATED FILL</h2>
                        <div className="flex justify-center">
                            <button className="bg-[#5A87FF] border-[6px] border-black rounded-3xl px-24 py-8 font-inter font-bold text-[50px] text-white hover:opacity-90">
                                Something Wrong Happened
                            </button>
                        </div>
                    </>
            );
        }
    }

    return (
        <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-20 h-[550px] flex flex-col justify-center">
            {renderContnet()}
        </div>
    );
}