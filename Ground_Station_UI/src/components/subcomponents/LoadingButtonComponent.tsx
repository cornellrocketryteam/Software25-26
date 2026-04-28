//Maybe delete I initially tested this and just find it quite useless
interface LoadingButtonComponentProps {buttonName: string; isSpecial?: boolean; 
    targetState: 'actuate' | 'close';  // What state we're transitioning to
    currentState: boolean; // Current state (true = actuated, false = closed)
  }
  
  export default function LoadingButtonComponent({ buttonName, isSpecial = false, targetState}: LoadingButtonComponentProps) {
    return (
      <div className="bg-white border-[6px] border-black rounded-3xl p-4 flex flex-col items-center justify-center">
        <p className="font-inter text-2xl mb-2">{buttonName}</p>
        
        <div className="flex gap-2">
          {/* Left side - OPEN/CLOSE or Special buttons (grayed out during loading) */}
          <div className="flex flex-col gap-2">
            {isSpecial ? (
              <button className="bg-[#555555]/30 border-[6px] border-black rounded-2xl px-8 py-3 font-inter font-bold text-2xl text-white/50 cursor-not-allowed"
              >
                Special
              </button>
            ) : (
              <>
                <button className="bg-[#ADC7AC]/30 border-[6px] border-black rounded-2xl px-8 py-3 font-inter font-bold text-2xl text-white/50 cursor-not-allowed"
                >
                  OPEN
                </button>
                <button className="bg-[#E27D7D]/30 border-[6px] border-black rounded-2xl px-8 py-3 font-inter font-bold text-2xl text-white/50 cursor-not-allowed"
                >
                  CLOSE
                </button>
              </>
            )}
          </div>
  
          {/* Right side - Loading indicator */}
          <div className="bg-[#FFA500] border-[6px] border-black rounded-2xl px-6 py-4 flex flex-col items-center justify-center min-w-[120px]">
            <p className="font-inter font-bold text-sm text-white mb-2">
              {targetState === 'actuate' ? 'ACTUATING...' : 'CLOSING...'}
            </p>
            
            {/* Spinner */}
            <div className="w-12 h-12 border-4 border-black rounded-full flex items-center justify-center relative">
              <div className="absolute inset-0 border-4 border-transparent border-t-black rounded-full animate-spin"></div>
              <div className="text-2xl font-bold">⟳</div>
            </div>
          </div>
        </div>
      </div>
    );
  }