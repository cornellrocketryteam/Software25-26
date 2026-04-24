import { useButton } from "../ButtonComponent";

export default function DisplayButtonComponent() {
    const {buttonName, currentState, stateLabel} = useButton();

    return (
      <div className="bg-white border-[6px] border-black rounded-3xl p-4 flex flex-col items-center justify-center">
        <p className="font-inter text-2xl mb-2">{buttonName}</p>
            <div className={`${currentState ? 'bg-[#ADC7AC]' : 'bg-[#E27D7D]'} border-[6px] border-black rounded-2xl px-8 py-6 flex flex-col items-center justify-center min-w-[200px]`}>
            <p className="font-inter font-bold text-sm text-white mb-2">
                State: {stateLabel}
            </p>
                <div className="w-12 h-12 border-4 border-black rounded-full flex items-center justify-center">
                    {currentState ? (
                    <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="black" strokeWidth="3">
                        {/* Check mark icon for closed state */}
                        <path d="M5 13l4 4L19 7" />
                    </svg>
                    ) : (
                    
                    <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="black" strokeWidth="3">
                        {/* X icon for closed state */}
                        <path d="M6 6l12 12M6 18L18 6" />
                    </svg>
                    )}
                </div>
            </div>
      </div>
    );
}