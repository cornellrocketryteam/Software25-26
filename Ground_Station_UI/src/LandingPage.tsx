import { useNavigate } from "react-router-dom";

export function LandingPage() {
    const navigate = useNavigate();

    return (
        <div className="min-h-screen bg-white">
        {/* Header */}
        <header className="bg-[#F9F9EB] px-8 py-4">
          <div className="flex items-center justify-center gap-4">
            <button onClick={() => navigate("/")} className="focus:outline-none">
                <img src="/src/assets/CRT_LOGO.png" alt="CRT Logo" className="h-16" />
            </button>
            <h1 className="text-4xl font-inter">Landing Page</h1>
          </div>
        </header>
      
        {/* Main Content */}
        <div className="flex items-center justify-center" style={{ minHeight: 'calc(100vh - 96px)' }}>
          {/* Gray Container */}
          <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-12 mx-auto">
            <div className="flex gap-[280px]">
                {/* Propulsion Page Button */}
                <button onClick={() => navigate("/propulsion")}
                className="bg-white border-[6px] border-black rounded-3xl px-16 py-20 text-4xl font-inter hover:bg-gray-50 transition-colors w-[500px] h-[250px] flex items-center justify-center">
                    Propulsion Page
                </button>
              
        
                {/* Recovery & Payload Page (Not clickable) */}
                <div className="bg-white border-[6px] border-black rounded-3xl px-16 py-20 text-4xl font-inter flex items-center justify-center w-[500px] h-[250px]">
                    Recovery & Payload Page
                </div>
            </div>
          </div>
        </div>
      </div>
    );
}

export default LandingPage;