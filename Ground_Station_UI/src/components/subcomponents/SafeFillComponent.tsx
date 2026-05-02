export default function SafeFillComponent() {
    return (
        <>
            <h2 className="font-inter font-bold text-[54px] text-center mb-12">INTERVENE WITH AUTOMATED FILL</h2>
            <div className="flex flex-col items-center gap-2">
                <button 
                    onClick={() => {
                        console.log("SAFE procedure already initiated.");
                    }}
                    className="bg-[#2D4556]/50 border-[6px] border-black rounded-3xl px-8 py-4 font-inter font-bold text-[48px] text-white/50 w-full max-w-[600px] cursor-not-allowed"
                >
                    <div className="flex flex-col items-center leading-tight">
                        <span>INITIATED SAFE</span>
                        <span>PROCEDURE</span>
                    </div>
                </button>
            </div>

        </>
    );
}