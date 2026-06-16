import { useNavigate } from "react-router-dom";

type StationCard = {
    title: string;
    description: string;
    route: string;
    accent: string;
};

const STATIONS: StationCard[] = [
    {
        title: "Propulsion",
        description: "Valve actuation, fill & vent control",
        route: "/propulsion",
        accent: "#5A87FF",
    },
    {
        title: "Recovery & Payload",
        description: "Target coordinates & payload deployment",
        route: "/recovery",
        accent: "#FF6B5A",
    },
];

export function LandingPage() {
    const navigate = useNavigate();

    return (
        <div className="min-h-screen bg-[#F5F5F5] flex flex-col">
            {/* Header */}
            <header className="bg-[#F9F9EB] px-8 py-4">
              <div className="flex items-center justify-center gap-4">
                <button onClick={() => navigate("/")} className="focus:outline-none">
                    <img src="/src/assets/CRT_LOGO.png" alt="CRT Logo" className="h-16" />
                </button>
                <h1 className="text-4xl font-inter font-semibold">Landing Page</h1>
              </div>
            </header>

            {/* Main Content */}
            <div className="flex-1 flex flex-col items-center justify-start px-8 pt-48 pb-12">
                {/* Description */}
                <div className="text-center mb-4">
                    <h2 className="text-5xl font-inter font-bold tracking-tight">Select a Station</h2>
                    <p className="mt-3 text-xl font-inter text-gray-600">
                        Choose a subsystem to monitor and command
                    </p>
                </div>

                {/* Station cards */}
                <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-10 shadow-2xl">
                    <div className="flex flex-row md:flex-row gap-10">
                        {STATIONS.map((station) => (
                            <button
                                key={station.route}
                                onClick={() => navigate(station.route)}
                                className="group bg-white border-[6px] border-black rounded-3xl w-[460px] max-w-full h-[300px] p-10 flex flex-col items-center justify-center gap-5 transition-all duration-200 hover:-translate-y-2 hover:shadow-[8px_8px_0_0_rgba(0,0,0,1)] focus:outline-none focus-visible:ring-4 focus-visible:ring-black/30"
                            >
                                <span className="text-4xl font-inter font-bold text-center">
                                    {station.title}
                                </span>
                                <span className="text-lg font-inter text-gray-500 text-center">
                                    {station.description}
                                </span>
                            </button>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
}

export default LandingPage;
