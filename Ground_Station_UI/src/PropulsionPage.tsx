import Header from "./components/HeaderComponent";
import ButtonComponent from "./components/ButtonComponent";
import VentButtonComponent from "./components/VentButtonComponent";
import FillButtonComponent from "./components/FillButtonComponent";
import HeaterPanelComponent from "./components/HeaterPanelComponent";
import { useEffect, useRef, useState } from "react";
import { createContext, useContext } from "react";
import { useAppContext } from "./App";


export type FillState = 'INITIAL' | 'INTERVENE' | 'SAFE_PROCEDURE' | 'STOP_FILL';
export type FlightMode = "....." | 'STANDBY';
export type actuationLockType = "LOCKED" | "UNLOCKED";
export type ActuationTypeIdentifier = 'OPEN' | 'CLOSE' | 'EXTEND' | 'RETRACT';
export type interactionType = "ENABLED" | "DISABLED";


type ValveData = {
    SV1: { "actuated": boolean, "continuity": boolean };
    SV2: { "actuated": boolean, "continuity": boolean };
    MAV: { "actuated": boolean, "angle": number, "pulseWidth": number };
    BV: { "actuated": boolean, "state": string };
}

type PropulsionContextType = {
    ventUIActive: boolean;
    setVentUIActive: (active: boolean) => void;
    fillUIActive: boolean;
    setFillUIActive: (active: boolean) => void;
    fillState: FillState;
    setFillState: (state: FillState) => void;
    ventSeconds: number;
    setVentSeconds: (seconds: number) => void;
    confirmedVentSeconds: number;
    manualVentRef: React.RefObject<boolean>;
    setConfirmedVentSeconds: (seconds: number) => void;
    valveData: ValveData;
    buttonInteractionState: interactionType;
    setButtonInteractionState: (allowInteraction: interactionType) => void;
    valveDataRef: React.RefObject<ValveData>;
    adcDataRef: React.RefObject<AdcDataMessage[]>;
    telemetryDataRef: React.RefObject<FswTelemetryMessage[]>;
    handleButtonClickRef: React.RefObject<(valveName: string, actuate: ActuationTypeIdentifier) => void>;
    confirmedVentSecondsRef: React.RefObject<number>;
    isVentingRef: React.RefObject<boolean>;
    isFillingRef: React.RefObject<boolean>;
    canInteractRef: React.RefObject<interactionType>;
    ventTimeoutRef: React.RefObject<ReturnType<typeof setTimeout> | null>;
}

type AdcDataMessage = {
    type: string;
    timestamp_ms: number;
    valid: boolean;
    adc1: AdcChannel[];
    adc2: AdcChannel[];
}

type Fsw_Telemetry = {
    flight_mode: number;
    pressure: number; //Outside pressure
    temp: number; //Outside temperature
    altitude: number;
    latitude: number;
    longitude: number;
    num_satellites: number;
    timestamp: number;
    mag_x: number;
    mag_y: number;
    mag_z: number;
    accel_x: number;
    accel_y: number;
    accel_z: number;
    gyro_x: number;
    gyro_y: number;
    gyro_z: number;
    pt3: number; 
    pt4: number; 
    rtd: number; 
}

type AdcChannel = {
    raw: number;
    voltage: number;
    scaled: number | null;
}

type FswTelemetryMessage = {
    type: string;
    timestamp_ms: number;
    connected: boolean;
    flight_mode: string;
    telemetry: Fsw_Telemetry;
}


type ValveKey = "SV1" | "SV2" | "MAV" | "IG1" | "IG2";

export const PropulsionContext = createContext<PropulsionContextType | null>(null);

export const usePropulsion = () => {
    const context = useContext(PropulsionContext);
    if (!context) throw new Error("usePropulsion must be used inside PropulsionPage (i.e., this useContext hook can only be called by components that are children of the PropulsionContext.Provider in the component tree)");
    return context;
};

export function PropulsionPage() {
    const { wsRef, wsReady, currFlightMode} = useAppContext(); //The App's websocket refrence

    const [fillState, setFillState] = useState<FillState>('INITIAL');
    const [ventSeconds, setVentSeconds] = useState(0);
    const [confirmedVentSeconds, setConfirmedVentSeconds] = useState(0);
    const [ventUIActive, setVentUIActive] = useState(false); // State to control whether the vent UI is active and visible
    const [fillUIActive, setFillUIActive] = useState(false); // State to control whether the fill UI is active and visible
    const [buttonInteractionState, setButtonInteractionState] = useState<interactionType>("DISABLED"); // State to control whether buttons can be interacted with, to prevent spamming commands while waiting for server responses


    const pendingActionRef = useRef<ValveKey | null>(null); // Use useRef to store the pending action for confirmation
    const adcDataRef = useRef<AdcDataMessage[]>([]); // Use useRef to store the latest ADC data received from the server
    const umbilicalDataRef = useRef<FswTelemetryMessage[]>([]); // Use useRef to store the latest telemetry data received from the server
    const manualVentRef = useRef(false); // We can use a ref for this since we don't necessarily need to trigger a re-render when this value changes
    const isVentingRef = useRef(ventUIActive); // Track whether a vent is currently in progress so the interval doesn't stack multiple vent cycles while the 1-second SV2 timeout is running.
    const isFillingRef = useRef(fillUIActive); // Mirror isFilling into a ref so the interval can check it synchronously without capturing a stale closure value.
    const canInteractRef = useRef<interactionType>(buttonInteractionState); // Ref to track whether buttons can be interacted with, to prevent spamming commands while waiting for server responses
    const ventTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null); // Stores the active vent timeout ID so it can be cancelled on abort
    const confirmedVentSecondsRef = useRef(confirmedVentSeconds); // Ref to track the confirmed vent seconds for the same reason as above

    /**
     * Below are examples of way to access elements in our refrence arrays
     * 
     * Access latest ADC or FSW data:
     * adcDataRef.current.at(-1)
     * umbilicalDataRef.current.at(-1)
     * 
     * Access the earliest ADC or FSW data:
     * adcDat.current.at(0)
     * umbilicalDataRef.current.at(0)
     * 
     */

    //Hearbeat command: send every 15 seconds to stay connected to server
    const heartbeatCommand = { "command": "heartbeat" };

    //Solenoid Valve Commands:
    const getSVstate1 = { "command": "get_valve_state", "valve": "SV1" };
    const actuateSV1Open = { "command": "actuate_valve", "valve": "SV1", "open": true };
    const actuateSV1Close = { "command": "actuate_valve", "valve": "SV1", "open": false };

    const actuateSV2Open = { "command": "fsw_open_sv" };
    const actuateSV2Close = { "command": "fsw_close_sv" };

    //Mav comands: 
    const actuateMavOpen = { "command": "fsw_open_mav" };
    const actuateMavClose = { "command": "fsw_close_mav" };

    //Ball Valve commands:
    const actuateBallValveOpen = { "command": "bv_open" };
    const actuateBallValveClose = { "command": "bv_close" };
    const getBallValveState = { "command": "get_ball_valve_state" };

    //Quick Disconnect commands:
    const actuateQDRetract = { "command": "qd_retract" };
    const actuateQDExtend = { "command": "qd_extend" };
    const getQdState = { "command": "get_qd_state" };


    //ADC stream commands:
    const startADCStream = { "command": "start_adc_stream" };
    const stopADCStream = { "command": "stop_adc_stream" };

    //FSW Umbilical Stream Commands:
    const startFSWStream = { "command": "start_fsw_stream" };
    const stopFSWStream = { "command": "stop_fsw_stream" };

    const queryCommands = ["get_valve_state", "get_ball_valve_state", "get_qd_state"]; //Only command for state query
    const actuationCommands = ["actuate_valve", "fsw_open_mav", "fsw_close_mav", "bv_open", "bv_close"]; //Set of commands that are used to change the state of the system


    //Stored Valve Data For UI Representation:
    
    const [valveData, setValveData] = useState({ //TODO: Think of what to do with venting field in valve Data
        SV1: { "actuated": false, "continuity": false },
        SV2: { "actuated": false, "venting": false, "continuity": false },
        MAV: { "actuated": false, "angle": 0, "pulseWidth": 0 },
        BV: { "actuated": false, "state": "high" },
        QD: { "retracted": false }
    });

    //Ref to hold the latest valve data for synchronous access in functions without worrying about stale closures, better for logic rather than UI representation
    const valveDataRef = useRef(valveData); 

    //Enforced base button delay for 250 miliseconds
    const buttondelay = 250; 

    //SEND COMMAND DELAY FUNCTION
    const sendCommandWithDelay = (param: any, delay: number) => {
        setTimeout(() => {
            if (typeof param === 'function') {
                param();
                return;
            }
            if (queryCommands.includes(param.command)) {
                pendingActionRef.current = param.valve;
            } else if (actuationCommands.includes(param.command)) {
                pendingActionRef.current = null;
            }
            wsRef.current?.send(JSON.stringify(param));
        }, delay);
    };

    /**
    The paramaeter `updater` takes in a function whose parameter is the previous state of the valve data and whose return value is the new state of the valve data, 
    which allows us to update our state based on the previous state in a safe way that avoids any issues with stale closures or asynchronous updates.
    */
    const updateValveData = (updater: (prev: typeof valveData) => typeof valveData) => {
        setValveData(prev => {
            const next = updater(prev); //Run the updater (spread operator) function and save it to next
            valveDataRef.current = next; //update our ref with the new state immeadiatly 
            return next; //return this new state
        });
    };

   
    const handleButtonClick = (ValveName: string, action: ActuationTypeIdentifier) => {

        const commandMap: { [key: string]: string } = { //---> map names of the buttons to shorthand identifiers
            "Solenoid Valve 1": "SV1",
            "Solenoid Valve 2": "SV2",
            "Ball Valve": "BV",
            "MAV": "MAV",
            "Quick Disconnect": "QD"
        }

        const valveIdentifier = commandMap[ValveName]; //<= This will map the button name to the corresponding valve identifier used in the command

        switch (valveIdentifier) {
            case "SV1":
                if (valveDataRef.current.SV1.actuated && action === 'CLOSE') {
                    sendCommandWithDelay(actuateSV1Close, buttondelay);
                } else if (!valveDataRef.current.SV1.actuated && action === 'OPEN') {
                    sendCommandWithDelay(actuateSV1Open, buttondelay);
                }

                sendCommandWithDelay(getSVstate1, buttondelay + 50); // Query the state of SV1 after sending the command to update our state with the response from the server
                console.log(`Toggling Solenoid Valve 1 to ${!valveDataRef.current.SV1.actuated ? 'OPEN' : 'CLOSED'}`);
                break;

            case "SV2":
                if (valveDataRef.current.SV2.actuated && action === 'CLOSE') {
                    sendCommandWithDelay(actuateSV2Close, buttondelay);
                } else if (!valveDataRef.current.SV2.actuated && action === 'OPEN') { 
                    sendCommandWithDelay(actuateSV2Open, buttondelay);
                }

                console.log(`Toggling Solenoid Valve 2 to ${!valveDataRef.current.SV2.actuated ? 'OPEN' : 'CLOSED'}`);
                break;

            case "MAV":
                if (valveDataRef.current.MAV.actuated && action === 'CLOSE') { 
                    sendCommandWithDelay(actuateMavClose, buttondelay);
                } else if (!valveDataRef.current.MAV.actuated && action === 'OPEN') { 
                    sendCommandWithDelay(actuateMavOpen, buttondelay); 
                }
                break;
            
            //For BV and QD, if the query commands for both aren't working, just fall back to the custom query function to update the state after the actuation command
            //Custom query commnds are behind though, so there will have to be some updates
            case "BV":
                if (valveDataRef.current.BV.actuated && action === 'CLOSE') { 
                    sendCommandWithDelay(actuateBallValveClose, buttondelay);
                    // sendCommandWithDelay(() => { //define custom query function to update state
                    //     updateValveData(prevState => ({
                    //         ...prevState,
                    //         BV: { actuated: false, state: "high" }
                    //     }));
                    //     console.log("Ball Valve toggled to CLOSED");
                    // }, buttondelay + 50); // Delay state update to give command time to execute
                    sendCommandWithDelay(getBallValveState, buttondelay + 50); // Query the state of the ball valve after sending the command to update our state with the response from the server
                } else if (!valveDataRef.current.BV.actuated && action === 'OPEN') {
                    sendCommandWithDelay(actuateBallValveOpen, buttondelay);
                    // sendCommandWithDelay(() => { //define custom query function to update state
                    //     updateValveData(prevState => ({
                    //         ...prevState,
                    //         BV: { actuated: true, state: "low" }
                    //     }));
                    //     console.log("Ball Valve toggled to OPEN");
                    // }, buttondelay + 50); // Delay state update to give command time to execute
                    sendCommandWithDelay(getBallValveState, buttondelay + 50); // Query the state of the ball valve after sending the command to update our state with the response from the server
                }
                break;

            case "QD":
                if (!valveDataRef.current.QD.retracted && action === 'RETRACT') {
                    sendCommandWithDelay(actuateQDRetract, buttondelay);
                    // sendCommandWithDelay(() => { //define custom query function to update state
                    //     updateValveData(prevState => ({
                    //         ...prevState,
                    //         QD: { ...prevState.QD, retracted: true }
                    //     }));
                    //     console.log("Quick Disconnect retracted");
                    // }, buttondelay + 50);
                    sendCommandWithDelay(getQdState, buttondelay + 50); // Query the state of the quick disconnect after sending the command to update our state with the response from the server
                } else if (valveDataRef.current.QD.retracted && action === 'EXTEND') {
                    sendCommandWithDelay(actuateQDExtend, buttondelay);
                    // sendCommandWithDelay(() => { //define custom query function to update state
                    //     updateValveData(prevState => ({
                    //         ...prevState,
                    //         QD: { ...prevState.QD, retracted: false }
                    //     }));
                    //     console.log("Quick Disconnect extended");
                    // }, buttondelay + 50);
                    sendCommandWithDelay(getQdState, buttondelay + 50); // Query the state of the quick disconnect after sending the command to update our state with the response from the server
                }
                break;

            default:
                console.error("Unknown valve identifier:", valveIdentifier);
        }
    };

    // This function will handle incoming messages from the WebSocket connection, and this will be set 
    // as the onmessage handler for our WebSocket in a useEffect hook.
    const handleMessage = (event: MessageEvent) => {
        //Parse JSON data here
        const data = JSON.parse(event.data);
        //console.log("Received message:", data); <- don't log so we can see pressure readings

        switch (data.type) { //Switch on response type 
            case "adc_data":
                if (adcDataRef.current.length > 2000) { //Limit the size of the ADC data array to prevent memory issues, adjust as needed based on how much data you want to keep track of
                    adcDataRef.current.shift(); //Remove the oldest entry when we exceed the limit
                }

                adcDataRef.current.push(data); //Store the latest ADC data in the ref
                break;

            case "fsw_telemetry":
                if (umbilicalDataRef.current.length > 500) { //Limit the size of the ADC data array to prevent memory issues, adjust as needed based on how much data you want to keep track of
                    umbilicalDataRef.current.shift(); //Remove the oldest entry when we exceed the limit
                }

                // For testing purposes so we can simulate fill without any actual pressure readings
                const lastPressure = umbilicalDataRef.current.at(-1)?.telemetry.pt3 ?? 0;

                if (isFillingRef.current && !isVentingRef.current) {
                    data.telemetry.pt3 = lastPressure + (Math.random() * 4 + 1); // Random increase between 1-5 during fill
                } else if (isVentingRef.current || (valveDataRef.current.SV2.venting && valveDataRef.current.BV.actuated)) {
                    data.telemetry.pt3 = Math.max(0, lastPressure - (Math.random() * 15 + 10)); // Random decrease between 10-25 during vent
                } else {
                    data.telemetry.pt3 = lastPressure; // Hold when idle
                }
                
                console.log("Pressure:", new Date().toISOString(), "PSI:", data.telemetry.pt3 ?? "N/A", "SV2 Open (possible vent):", data.telemetry.sv_open ?? "N/A"); //Log pressure and venting status for testing

                umbilicalDataRef.current.push(data); //Store the latest umbilical data in the ref

                // Update MAV state based on telemetry
                if (data.telemetry && data.telemetry.mav_open !== undefined) {
                    updateValveData(prevState => ({
                        ...prevState,
                        MAV: { "actuated": data.telemetry.mav_open, "angle": 0, "pulseWidth": 0 }
                    }));
                }
                // Update SV2 state based on telemetry, including venting status and continuity if available
                if (data.telemetry && data.telemetry.sv_open !== undefined) {
                    updateValveData(prevState => ({
                        ...prevState,
                        SV2: { ...prevState.SV2, "actuated": data.telemetry.sv_open, "venting": data.telemetry.sv_open }
                    }));
                }
                break;


            case "valve_state":
                if (pendingActionRef.current === "SV1") {
                    updateValveData(prevState => ({
                        ...prevState,
                        SV1: { "actuated": data.open, "continuity": data.continuity }
                    }));
                }
                break;

            case "ball_valve_state":
                updateValveData(prevState => ({
                    ...prevState,
                    BV: { ...prevState.BV, actuated: data.open }
                }));
                break;

            case "qd_state":
                updateValveData(prevState => ({
                    ...prevState,
                    QD: { retracted: data.state === -1 }
                }));
                break;
        }
    };


    /*
    Reason why we added the handleButtonClickRef (Claude)
    Every re-render creates a new version of handleButtonClick. Your setTimeout captures the old version at creation time. When it fires later,
    it uses that old version which has a stale wsRef → WebSocket error → disconnect. By using a ref to always point to the latest handleButtonClick, 
    the setTimeout can call handleButtonClickRef.current() to get the up-to-date function with the current wsRef, preventing the stale closure issue and keeping the connection alive.
    */
    const handleButtonClickRef = useRef(handleButtonClick);

    useEffect(() => {
        handleButtonClickRef.current = handleButtonClick;
    }); // No dependency array, runs after every render, keeping ref always fresh
    useEffect(() => {
        confirmedVentSecondsRef.current = confirmedVentSeconds;
    }, [confirmedVentSeconds]); //Runs after every change of converVentSeconds, keeping ref always fresh

    //useEffect hook sets up connection to fill station server on mount of propulsion page, and also sets up listeners for messages, connection closures, and errors, 
    //and then cleans up the connection on unmount of the page. The connection closure listener also attempts to reconnect after a delay if the connection is lost, and 
    //the error listener ensures that any errors also trigger a closure and reconnection attempt.
    useEffect(() => {
        if (!wsReady) return; // Connection not open yet, do nothing

        let heartbeatInterval: ReturnType<typeof setInterval>;
        let pollingInterval: ReturnType<typeof setInterval>;

        const onOpen = () => {
            sendCommandWithDelay(getSVstate1, 50);
            sendCommandWithDelay(getBallValveState, 50);
            sendCommandWithDelay(getQdState, 50);
            sendCommandWithDelay(heartbeatCommand, buttondelay);
            sendCommandWithDelay(startADCStream, buttondelay);
            sendCommandWithDelay(startFSWStream, buttondelay);
            console.log("Command batch finished");

            heartbeatInterval = setInterval(() => {
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                    wsRef.current.send(JSON.stringify(heartbeatCommand));
                }
            }, 5000);

            pollingInterval = setInterval(() => {
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                    sendCommandWithDelay(getSVstate1, 50);
                    sendCommandWithDelay(getBallValveState, 50);
                    sendCommandWithDelay(getQdState, 50);
                }
            }, 3000);
        };

        if (wsRef.current?.readyState === WebSocket.OPEN) {
            onOpen();
        } else {
            wsRef.current?.addEventListener('open', onOpen);
        }

        // Set up message listener to handle incoming messages from the server and update our state accordingly
        wsRef.current?.addEventListener('message', handleMessage);

        return () => {
            clearInterval(heartbeatInterval);
            clearInterval(pollingInterval);
            wsRef.current?.removeEventListener('open', onOpen);
            wsRef.current?.removeEventListener('message', handleMessage);
            if (wsRef.current?.readyState === WebSocket.OPEN) {
                wsRef.current.send(JSON.stringify(stopADCStream));
                wsRef.current.send(JSON.stringify(stopFSWStream));
            }
            console.log("Propulsion cleanup.");
        };
    }, [wsReady]); // Re-run effect if WebSocket connection status changes

    return (
        <PropulsionContext.Provider value={{ ventTimeoutRef, confirmedVentSecondsRef, canInteractRef, buttonInteractionState, setButtonInteractionState, valveDataRef, fillUIActive, setFillUIActive, ventUIActive, setVentUIActive, isVentingRef, isFillingRef, manualVentRef, handleButtonClickRef, fillState, setFillState, ventSeconds, setVentSeconds, confirmedVentSeconds, setConfirmedVentSeconds, valveData, adcDataRef: adcDataRef, telemetryDataRef: umbilicalDataRef }}>
            <div className={`min-h-screen bg-white ${ventUIActive ? 'cursor-wait' : ''}`}>
                {/* Header */}
                <Header
                    pageTitle="Propulsion Page"
                    currFlightMode={currFlightMode}
                    buttonInteractionState={buttonInteractionState}
                    canInteractRef={canInteractRef}
                    fillUIActive={fillUIActive}
                    setButtonInteractionState={setButtonInteractionState}
                />

                {/* Main Content */}
                <div className="flex gap-5 p-8">
                    {/* Left Column */}
                    <div className="flex-1 flex flex-col gap-8">
                        {/* START AUTOMATED FILL */}
                        <FillButtonComponent />

                        {/* VENT BUTTON */}
                        <VentButtonComponent />
                    </div>

                    {/* Right Column */}
                    <div className="flex-1 flex flex-col gap-8">
                        {/* 6 Button Grid */}
                        <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-5">
                            <div className="grid grid-cols-2 gap-[25px]">
                                <ButtonComponent buttonName="Solenoid Valve 1" currentState={valveData.SV1.actuated} actuationLock='LOCKED' />
                                <ButtonComponent buttonName="Solenoid Valve 2" currentState={valveData.SV2.actuated} actuationLock='LOCKED' />
                                <ButtonComponent buttonName="Ball Valve" currentState={valveData.BV.actuated} actuationLock='UNLOCKED' />
                                <ButtonComponent buttonName="MAV" currentState={valveData.MAV.actuated} actuationLock='LOCKED' />
                                <ButtonComponent buttonName="Quick Disconnect" currentState={valveData.QD.retracted} actuationLock='UNLOCKED' />
                            </div>

                        </div>



                        {/* Home Assistant Tank Heaters */}
                        <HeaterPanelComponent />
                    </div>
                </div>
            </div>
        </PropulsionContext.Provider>
    );
}

export default PropulsionPage;