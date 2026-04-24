import { useEffect, useRef, useState } from "react";

const HA_TOKEN = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiIzZTdmMzY4NWNjYWI0MjcyOTVmNzg2Zjg2NTJkYWMwNCIsImlhdCI6MTc3NzAzMDYxOCwiZXhwIjoyMDkyMzkwNjE4fQ.BgmgDVTUse5TIe1OYRaZ6J2el0khu_FYGu4ZtFHlzh8";
const ENTITIES = ["switch.tankheater1", "switch.tankheater2", "switch.tankheater3"];

export default function HeaterPanelComponent() {
    const wsRef = useRef<WebSocket | null>(null);
    const [states, setStates] = useState<Record<string, boolean>>({
        "switch.tankheater1": false,
        "switch.tankheater2": false,
        "switch.tankheater3": false
    });
    const [connected, setConnected] = useState(false);
    const msgIdRef = useRef(1);

    useEffect(() => {
        let reconnectTimeout: ReturnType<typeof setTimeout>;

        const connect = () => {
            const host = window.location.hostname || "192.168.8.167";
            const uri = `ws://${host}:8123/api/websocket`;
            console.log("Connecting to Home Assistant at", uri);
            wsRef.current = new WebSocket(uri);

            wsRef.current.onopen = () => {
                console.log("HA WebSocket connected.");
            };

            wsRef.current.onmessage = (event) => {
                const data = JSON.parse(event.data);
                
                if (data.type === "auth_required") {
                    wsRef.current?.send(JSON.stringify({
                        type: "auth",
                        access_token: HA_TOKEN
                    }));
                } else if (data.type === "auth_invalid") {
                    console.error("HA Authentication Failed");
                } else if (data.type === "auth_ok") {
                    setConnected(true);
                    
                    // Subscribe to state changes
                    wsRef.current?.send(JSON.stringify({
                        id: msgIdRef.current++,
                        type: "subscribe_events",
                        event_type: "state_changed"
                    }));

                    // Get initial states
                    wsRef.current?.send(JSON.stringify({
                        id: msgIdRef.current++,
                        type: "get_states"
                    }));
                } else if (data.type === "event" && data.event.event_type === "state_changed") {
                    const entity_id = data.event.data.entity_id;
                    if (ENTITIES.includes(entity_id)) {
                        setStates(prev => ({
                            ...prev,
                            [entity_id]: data.event.data.new_state.state === "on"
                        }));
                    }
                } else if (data.type === "result" && Array.isArray(data.result)) {
                    // Initial states result
                    const initialStates = { ...states };
                    data.result.forEach((entity: any) => {
                        if (ENTITIES.includes(entity.entity_id)) {
                            initialStates[entity.entity_id] = entity.state === "on";
                        }
                    });
                    setStates(initialStates);
                }
            };

            wsRef.current.onclose = () => {
                setConnected(false);
                console.log("HA WebSocket closed. Reconnecting...");
                reconnectTimeout = setTimeout(connect, 5000);
            };

            wsRef.current.onerror = (err) => {
                console.error("HA WebSocket error", err);
                wsRef.current?.close();
            };
        };

        connect();

        return () => {
            clearTimeout(reconnectTimeout);
            if (wsRef.current) {
                wsRef.current.onclose = null;
                wsRef.current.onerror = null;
                wsRef.current.close();
            }
        };
    }, []);

    const toggleHeater = (entity_id: string, turnOn: boolean) => {
        if (!wsRef.current || !connected) return;
        
        wsRef.current.send(JSON.stringify({
            id: msgIdRef.current++,
            type: "call_service",
            domain: "switch",
            service: turnOn ? "turn_on" : "turn_off",
            target: { entity_id }
        }));
    };

    const HeaterButton = ({ name, entity_id }: { name: string, entity_id: string }) => {
        const isOpen = states[entity_id];
        return (
            <div className="bg-white border-[6px] border-black rounded-3xl p-4 flex flex-col items-center justify-center w-full overflow-hidden">
                <p className="font-inter text-2xl mb-2">{name}</p>
                <div className="flex gap-2">
                    <div className="flex flex-col gap-2 min-w-0 w-full">
                        <button
                            onClick={() => toggleHeater(entity_id, true)}
                            className={`${isOpen ? 'bg-[#ADC7AC]/50 cursor-not-allowed opacity-50' : 'bg-[#ADC7AC]'} border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                        >
                            ON
                        </button>
                        <button
                            onClick={() => toggleHeater(entity_id, false)}
                            className={`${!isOpen ? 'bg-[#E27D7D]/50 cursor-not-allowed opacity-50' : 'bg-[#E27D7D]'} border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                        >
                            OFF
                        </button>
                    </div>
                    <div className={`${isOpen ? 'bg-[#ADC7AC]' : 'bg-[#E27D7D]'} border-[6px] border-black rounded-2xl px-6 py-4 flex flex-col items-center justify-center min-w-[120px]`}>
                        <p className="font-inter font-bold text-sm text-white mb-2">
                            State: {isOpen ? "ON" : "OFF"}
                        </p>
                        <div className="w-12 h-12 border-4 border-black rounded-full flex items-center justify-center">
                            {isOpen ? (
                                <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="black" strokeWidth="3">
                                    <path d="M5 13l4 4L19 7" />
                                </svg>
                            ) : (
                                <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="black" strokeWidth="3">
                                    <path d="M6 6l12 12M6 18L18 6" />
                                </svg>
                            )}
                        </div>
                    </div>
                </div>
            </div>
        );
    };

    return (
        <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-5 mt-8">
            <div className="flex justify-between items-center mb-4">
                <h2 className="font-inter text-2xl font-bold">Tank Heaters</h2>
                <div className="flex items-center gap-2">
                    <span className="font-inter text-sm font-bold">HA Status:</span>
                    <div className={`w-4 h-4 rounded-full border-2 border-black ${connected ? 'bg-green-500' : 'bg-red-500'}`}></div>
                </div>
            </div>
            <div className="grid grid-cols-1 gap-[25px]">
                <HeaterButton name="Heater 1" entity_id="switch.tankheater1" />
                <HeaterButton name="Heater 2" entity_id="switch.tankheater2" />
                <HeaterButton name="Heater 3" entity_id="switch.tankheater3" />
            </div>
        </div>
    );
}
