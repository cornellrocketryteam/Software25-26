export default function StopFillComponent() {
    return (
        <>
            <h2 className="font-inter font-bold text-[54px] text-center mb-12">INTERVENE WITH AUTOMATED FILL</h2>
            <div className="flex flex-col items-center gap-2">
                <button 
                    onClick={() => {
                        console.log("STOP FILL has been initiated.");
                    }}
                    className="bg-[#1A1A1A]/50 border-[6px] border-black rounded-3xl px-8 py-2 font-inter font-bold text-[48px] text-white/50 w-full max-w-[600px] cursor-not-allowed"
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