import Header from "./components/HeaderComponent";
import ButtonComponent from "./components/ButtonComponent";
import VentButtonComponent from "./components/VentButtonComponent";
import FillButtonComponent from "./components/FillButtonComponent";
import ButtonPanelComponent from "./components/ButtonPanelComponent";
import HeaterPanelComponent from "./components/HeaterPanelComponent";
import {useEffect, useRef, useState} from "react";
import { createContext, useContext} from "react";
import { useAppContext } from "./App";


export type FillState = 'INITIAL' | 'INTERVENE' | 'SAFE_PROCEDURE' | 'STOP_FILL';
export type FlightMode = "....."| 'STANDBY';
export type actuationLockType = "LOCKED" | "UNLOCKED";
export type ActuationTypeIdentifier = 'OPEN' | 'CLOSE' | 'IGNITE' | 'EXTEND' | 'RETRACT';
export type interactionType = "ENABLED" | "DISABLED";


type ValveData = {
    SV1: {"actuated": boolean, "continuity": boolean};
    SV2: {"actuated": boolean, "continuity": boolean};
    MAV: {"actuated": boolean, "angle": number, "pulseWidth": number};
    BV:  {"actuated": boolean, "state": string};
    IG1: {"continuity": boolean};
    IG2: {"continuity": boolean};
}

type PropulsionContextType = {
    thresholdPressure: number;
    setThresholdPressure: (pressure: number) => void;
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
    fswConnected: boolean;
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
    pt3: number; //Injector Pressure
    pt4: number; //Runtank Pressure <-- What I really care about
    rtd: number; //Temperature of the runtank
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
    const {wsRef, wsReady, currFlightMode, setCurrFlightMode} = useAppContext(); //The App's websocket refrence

    const [fillState, setFillState] = useState<FillState>('INITIAL');
    const [fswConnected, setFswConnected] = useState(false);
    const [ventSeconds, setVentSeconds] = useState(0);
    const [confirmedVentSeconds, setConfirmedVentSeconds] = useState(0);
    const [thresholdPressure, setThresholdPressure] = useState(600); 
    const [ventUIActive, setVentUIActive] = useState(false); // State to control whether the vent UI is active and visible
    const [fillUIActive, setFillUIActive] = useState(false); // State to control whether the fill UI is active and visible
    const [buttonInteractionState, setButtonInteractionState] = useState<interactionType>("DISABLED"); // State to control whether buttons can be interacted with, to prevent spamming commands while waiting for server responses


    
    //const uri = "ws://192.168.8.167:9000";
    //const uri = "ws://localhost:9000"; // Replace with the WebSocket server URI
    //const wsRef = useRef<WebSocket | null>(null); // Use useRef to store the WebSocket instance
    const pendingActionRef = useRef<ValveKey | null>(null); // Use useRef to store the pending action for confirmation
    const adcDataRef = useRef<AdcDataMessage[]>([]); // Use useRef to store the latest ADC data received from the server
    const umbilicalDataRef = useRef<FswTelemetryMessage[]>([]); // Use useRef to store the latest telemetry data received from the server
    const manualVentRef = useRef(false); // We can use a ref for this since we don't necessarily need to trigger a re-render when this value changes
    const isVentingRef = useRef(ventUIActive); // Track whether a vent is currently in progress so the interval doesn't stack multiple vent cycles while the 1-second SV2 timeout is running.
    const isFillingRef = useRef(fillUIActive); // Mirror isFilling into a ref so the interval can check it synchronously without capturing a stale closure value.
    const canInteractRef = useRef<interactionType>(buttonInteractionState); // Ref to track whether buttons can be interacted with, to prevent spamming commands while waiting for server responses
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
    const heartbeatCommand = {"command": "heartbeat"};

    //Igniter commands: get the continuity status of the igniters, likely will need to be reworked to fit the actual data being sent by the server and how we want to handle it. 
    //If we have continuity then we have the ability to ignite
    const getIgniterContinuity1 = {"command": "get_igniter_continuity", "id": 1}; //Just to log the continuity status of igniter 1 for now, but may want to use this information to update some state and display it on the page or use it for some logic in the future
    const getIgniterContinuity2 = {"command": "get_igniter_continuity", "id": 2}; //Just to log the continuity status of igniter 2 for now, but may want to use this information to update some state and display it on the page or use it for some logic in the future
    const igniteCommand = {"command": "ignite"};
    const launchCommand = {"command": "fsw_launch"};


    //Solenoid Valve Commands: Get the information of the valve states on mount (so call in useEffect) so I can set my initial state of the valve buttons to 
    //match the actual state of the valves, and then also use these commands to get updated valve states after sending any command that would change the valve states.
    const getSVstate1 = {"command": "get_valve_state", "valve": "SV1"};
    // = {"command": "get_valve_state", "valve": "SV2"};
    const actuateSV1Open = {"command": "actuate_valve", "valve": "SV1", "open": true};
    const actuateSV1Close = {"command": "actuate_valve", "valve": "SV1", "open": false};
    //const actuateSV2Open = {"command": "actuate_valve", "valve": "SV2", "open": true};
    //const actuateSV2Close = {"command": "actuate_valve", "valve": "SV2", "open": false};

    const actuateSV2Open = {"command": "fsw_open_sv"};
    const actuateSV2Close = {"command": "fsw_close_sv"};



    //Mav comands: 
    const actuateMavOpen = {"command": "fsw_open_mav"};
    const actuateMavClose = {"command": "fsw_close_mav"};

    //Ball Valve commands: 
    //There is no get state command for the ball valve, so I will just have to keep track of the state based on the commands I send to it and assume they work correctly, 
    //unless we want to add some sort of sensor for it in the future (set default to closed since that's the safe state for it to be in per the confluence page)
    const actuateBallValveOpen = {"command": "bv_open"};
    const actuateBallValveClose = {"command": "bv_close"};

    //Quick Disconnect commands:
    const actuateQDOpen = {"command": "qd_retract"}; 
    const actuateQDClose = {"command": "qd_extend"};


    //Start and stop ADC stream commands: will need to talk to Ronit about how to implement this, or even if I have to, but the idea is that I would send the start 
    //command in useEffect after establishing the WebSocket connection to start receiving the ADC data, and then send the stop command in the clean up function of useEffect 
    //to stop receiving the data when the component unmounts. Depending on how the server is set up, I may also need to set up a listener for incoming ADC data and update 
    //some state with that data to display it on the page or use it for some logic.
    const startADCStream = {"command": "start_adc_stream"};
    const stopADCStream = {"command": "stop_adc_stream"};

    //FSW Umbilical Commands: set up FSW stream to get FSW telemetry (For now, just learn how to keep track of the data we get from this stream)
    const startFSWStream = {"command": "start_fsw_stream"};
    const stopFSWStream = {"command": "stop_fsw_stream"};

    const queryCommands = ["get_valve_state", "get_igniter_continuity"]; //Set of commands that are used to get information about the current state of the system, which we want to track as pending actions so that when we get the response back from the server, we know what information it corresponds to and can update our state accordingly
    const actuationCommands = ["actuate_valve", "fsw_open_mav", "fsw_close_mav", "bv_open", "bv_close", "ignite"]; //Set of commands that are used to change the state of the system, which we want to clear any pending actions for when we send them, since we know that any information we get back from the server after sending one of these commands will be outdated and not relevant to our current state


    //Stored Valve Data
    //This is where we will store the current state of the valves and igniters based on the responses we get back from the server when we send the query commands, so that we can use that information to update our 
    //button states and display the current state of the system on the page. We will update this state whenever we get a response back from the server for one of our query commands, and we will also use this state to 
    //determine what command to send when a button is clicked (e.g., if SV1 is currently open according to our state, then when we click the SV1 button, we know we need to send the command to close it).
    const [valveData, setValveData] = useState({
        SV1: {"actuated": false, "continuity": false},
        SV2: {"actuated": false, "venting" : false , "continuity": false},
        MAV: {"actuated": false, "angle": 0, "pulseWidth": 0},
        BV:  {"actuated": false, "state": "high"},
        IG1: {"continuity": false},
        IG2: {"continuity": false},
        QD: {"retracted": false}
    });

    //Implement this refrence for all my logic and also make sure that whenever I update my valveData state, I also update this ref with the latest data so that I can access the most up to date valve data in my functions without worrying about stale closures from useState
    const valveDataRef = useRef(valveData); // Create a ref to hold the latest valve data for synchronous access in functions without worrying about stale closures

    //Do we want a delay? This is important before testing ;)


    const buttondelay = 250; // Example delay in milliseconds between commands, can adjust as needed based on testing and how quickly the server can process commands and send responses

    //COMMAND DELAY FUNCTION
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

    //This function is called when a valve button is clicked, and it determines which command to send based on the name of the button and the current state of the corresponding valve in our state. 
    //It also sets the pending action for query commands so that we can update our state with the response from the server when we get it back.
    const handleButtonClick = (ValveName: string, action: ActuationTypeIdentifier) => { 

        const commandMap : {[key: string]: string} = { //---> map names of the buttons to shorthand identifiers
            "Solenoid Valve 1": "SV1",
            "Solenoid Valve 2": "SV2",
            "Ball Valve": "BV",
            "Igniter": "Igniter",
            "MAV": "MAV",
            "Quick Disconnect": "QD",
            "LAUNCH": "LAUNCH"
        }

        const valveIdentifier = commandMap[ValveName]; //<= This will map the button name to the corresponding valve identifier used in the command

        switch (valveIdentifier) { //Handles logic for button presses based of the current state of the button
            case "SV1":
                if (valveDataRef.current.SV1.actuated && action === 'CLOSE') { // If SV1 is currently open and we want to close it
                    sendCommandWithDelay(actuateSV1Close, buttondelay);   
                } else if (!valveDataRef.current.SV1.actuated && action === 'OPEN') { // If SV1 is currently closed and we want to open it
                    sendCommandWithDelay(actuateSV1Open, buttondelay);
                }

                sendCommandWithDelay(getSVstate1, buttondelay + 50); // Query the state of SV1 after sending the command to update our state with the response from the server
                console.log(`Toggling Solenoid Valve 1 to ${!valveDataRef.current.SV1.actuated ? 'OPEN' : 'CLOSED'}`);
                break;
        
            case "SV2":
                if (valveDataRef.current.SV2.actuated && action === 'CLOSE') { // If SV1 is currently open and we want to close it
                    sendCommandWithDelay(actuateSV2Close, buttondelay);   
                } else if (!valveDataRef.current.SV2.actuated && action === 'OPEN') { // If SV1 is currently closed and we want to open it
                    sendCommandWithDelay(actuateSV2Open, buttondelay);
                }

                //sendCommandWithDelay(getSVstate2, buttondelay + 50); // Query the state of SV1 after sending the command to update our state with the response from the server
                console.log(`Toggling Solenoid Valve 2 to ${!valveDataRef.current.SV2.actuated ? 'OPEN' : 'CLOSED'}`);
                break;
        
            case "BV":
                if (valveDataRef.current.BV.actuated && action === 'CLOSE') { // If the ball valve is currently open and we want to close it
                    sendCommandWithDelay(actuateBallValveClose, buttondelay);
                    sendCommandWithDelay(() => {
                        updateValveData(prevState => ({
                            ...prevState,
                            BV: { actuated: false, state: "high" }
                        }));
                        console.log("Ball Valve toggled to CLOSED");
                    }, buttondelay + 50); // Delay state update to give command time to execute
                } else if (!valveDataRef.current.BV.actuated && action === 'OPEN') { // If the ball valve is currently open and we want to open it (i.e., do nothing)
                    sendCommandWithDelay(actuateBallValveOpen, buttondelay);
                    sendCommandWithDelay(() => {
                        updateValveData(prevState => ({
                            ...prevState,
                            BV: { actuated: true, state: "low" }
                        }));
                        console.log("Ball Valve toggled to OPEN");
                    }, buttondelay + 50); // Delay state update to give command time to execute
                }
                break;
        
            case "MAV":
                if(valveDataRef.current.MAV.actuated && action === 'CLOSE') { //If the Mav is actuated then we want to close it
                    sendCommandWithDelay(actuateMavClose, buttondelay); //Send `Close` Command for Mav Valve
                }else if(!valveDataRef.current.MAV.actuated && action === 'OPEN') { //If the Mav is not actuated then we want to open it
                    sendCommandWithDelay(actuateMavOpen, buttondelay); //Send `Open` Command for Mav Valve
                }
                break;
        
            case "QD":
                if (valveDataRef.current.QD.retracted && action === 'RETRACT') {
                    sendCommandWithDelay(actuateQDClose, buttondelay);
                    sendCommandWithDelay(() => {
                        updateValveData(prevState => ({
                            ...prevState,
                            QD: { ...prevState.QD, retracted: false }
                        }));
                        console.log("Quick Disconnect toggled to CLOSED");
                    }, buttondelay + 50);
                } else if (!valveDataRef.current.QD.retracted && action === 'EXTEND'){
                    sendCommandWithDelay(actuateQDOpen, buttondelay);
                    sendCommandWithDelay(() => {
                        updateValveData(prevState => ({
                            ...prevState,
                            QD: { ...prevState.QD, retracted: true }
                        }));
                        console.log("Quick Disconnect toggled to OPEN");
                    }, buttondelay + 50);
                }
                break;
        
            case "Igniter":
                if (valveDataRef.current.IG1.continuity && valveDataRef.current.IG2.continuity) {
                    sendCommandWithDelay(igniteCommand, buttondelay);
                } else {
                    const missingContinuity = [
                        !valveDataRef.current.IG1.continuity && "IG1",
                        !valveDataRef.current.IG2.continuity && "IG2"
                    ].filter(Boolean).join(", ");
                    console.warn(`Ignite command blocked — no continuity on: ${missingContinuity}`);
                }
                break;

            case "LAUNCH":
                // Send FSW launch signal first, then fire igniters immediately after
                sendCommandWithDelay(launchCommand, buttondelay);
                sendCommandWithDelay(igniteCommand, buttondelay + 50);
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
        //console.log("Received message:", data);
    
        switch (data.type) { //Switch on response type 
            case "adc_data":
                if(adcDataRef.current.length > 2000) { //Limit the size of the ADC data array to prevent memory issues, adjust as needed based on how much data you want to keep track of
                    adcDataRef.current.shift(); //Remove the oldest entry when we exceed the limit
                }
    
                adcDataRef.current.push(data); //Store the latest ADC data in the ref, which we can use for displaying on the page or for any logic we want to implement based on the ADC data in the future
                break;
    
            case "fsw_telemetry":
                setFswConnected(data.connected);
                if(umbilicalDataRef.current.length > 500) {
                    umbilicalDataRef.current.shift();
                }
                

                //const lastPressure = umbilicalDataRef.current.at(-1)?.telemetry.pt3 ?? 0;
    
                // if (isFillingRef.current && !isVentingRef.current) {
                //     data.telemetry.pt3 = lastPressure + (Math.random() * 4 + 1); // Random increase between 1-5 during fill
                // } else if (isVentingRef.current || (valveDataRef.current.SV2.venting && valveDataRef.current.BV.actuated)) {
                //     data.telemetry.pt3 = Math.max(0, lastPressure - (Math.random() * 15 + 10)); // Random decrease between 10-25 during vent
                // } else {
                //     data.telemetry.pt3 = lastPressure; // Hold when idle
                // }
    
                console.log("Pressure:", new Date().toISOString(), "PSI:", data.telemetry.pt3);
    
                setCurrFlightMode(data.flight_mode as FlightMode); //Update our current flight mode state with the latest flight mode from the telemetry data, which we can use to display on the page or for any logic we want to implement based on the flight mode in the future
                umbilicalDataRef.current.push(data); //Store the latest ADC data in the ref, which we can use for displaying on the page or for any logic we want to implement based on the ADC data in the future
                
                // Update MAV state based on telemetry
                if (data.telemetry && data.telemetry.mav_open !== undefined) {
                    updateValveData(prevState => ({
                        ...prevState,
                        MAV: { "actuated": data.telemetry.mav_open, "angle": 0, "pulseWidth": 0 }
                    }));
                }
                if (data.telemetry && data.telemetry.sv_open !== undefined) {
                    updateValveData(prevState => ({
                        ...prevState, //Spread the previous state to keep other valves' data unchanged
                        SV2: { "actuated": data.telemetry.sv_open, "venting": data.telemetry.sv_open, "continuity": data.continuity }
                        //Adjust as needed based on actual response data and what information we want to track
                    }));
                }
                break;
    
            case "igniter_continuity":
                if (data.id === 1) {
                    updateValveData(prevState => ({
                        ...prevState, //Spread the previous state to keep other valves' data unchanged
                        IG1: { "continuity": data.continuity }
                        //Adjust as needed based on actual response data and what information we want to track
                    }));
                }
                else if (data.id === 2) {
                    updateValveData(prevState => ({
                        ...prevState, //Spread the previous state to keep other valves' data unchanged
                        IG2: { "continuity": data.continuity }
                        //Adjust as needed based on actual response data and what information we want to track
                    }));
                }
    
            break;
            
            case "valve_state":
                //Check pending action to see which valve this response is for and update state accordingly
                if (pendingActionRef.current === "SV1") {
                    updateValveData(prevState => ({
                        ...prevState, //Spread the previous state to keep other valves' data unchanged
                        SV1: { "actuated": data.actuated, "continuity": data.continuity }
                        //Adjust as needed based on actual response data and what information we want to track
                    }));
                }
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
        }); // No dependency array = runs after every render, keeping ref always fresh
        
        //useEffect hook sets up connection to fill station server on mount of propulsion page, and also sets up listeners for messages, connection closures, and errors, 
        //and then cleans up the connection on unmount of the page. The connection closure listener also attempts to reconnect after a delay if the connection is lost, and 
        //the error listener ensures that any errors also trigger a closure and reconnection attempt.
        useEffect(() => {
            if (!wsReady) return; // Connection not open yet, do nothing
    
            let heartbeatInterval: ReturnType<typeof setInterval>; 
            let pollingInterval: ReturnType<typeof setInterval>;
        
            const onOpen = () => {
                sendCommandWithDelay(getSVstate1, 0);
                //sendCommandWithDelay(getSVstate2, 50);
                sendCommandWithDelay(getIgniterContinuity1, 50);
                sendCommandWithDelay(getIgniterContinuity2, 50);
                sendCommandWithDelay(heartbeatCommand, 50);
                //sendCommandWithDelay(getSVstate2, 150);
                sendCommandWithDelay(() => {
                    if (!valveDataRef.current.SV2.actuated) {
                        wsRef.current?.send(JSON.stringify(actuateSV2Open));
                        //sendCommandWithDelay(getSVstate2, 50);
                    }
                    if (valveDataRef.current.MAV.actuated) {
                        wsRef.current?.send(JSON.stringify(actuateMavClose));
                    }
                }, 400);
                sendCommandWithDelay(startADCStream, 333);
                sendCommandWithDelay(startFSWStream, 333);
                console.log("Command batch finished");
        
                heartbeatInterval = setInterval(() => {
                    if (wsRef.current?.readyState === WebSocket.OPEN) {
                        wsRef.current.send(JSON.stringify(heartbeatCommand));
                    }
                }, 5000);
        
                pollingInterval = setInterval(() => {
                    if (wsRef.current?.readyState === WebSocket.OPEN) {
                        sendCommandWithDelay(getSVstate1, 0);
                        //sendCommandWithDelay(getSVstate2, 50);
                        sendCommandWithDelay(getIgniterContinuity1, 150);
                        sendCommandWithDelay(getIgniterContinuity2, 200);
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
            <PropulsionContext.Provider value={{confirmedVentSecondsRef, canInteractRef, buttonInteractionState, setButtonInteractionState, valveDataRef, fillUIActive, setFillUIActive, ventUIActive, setVentUIActive, isVentingRef, isFillingRef, manualVentRef, handleButtonClickRef, fillState, setFillState, thresholdPressure, setThresholdPressure, ventSeconds, setVentSeconds, confirmedVentSeconds, setConfirmedVentSeconds, valveData, adcDataRef: adcDataRef, telemetryDataRef: umbilicalDataRef, fswConnected}}>
                <div className={`min-h-screen bg-white ${ventUIActive ? 'cursor-wait' : ''}`}>
                    {/* Header */}       
                <Header 
                pageTitle="Propulsion Page" 
                currFlightMode={currFlightMode} 
                buttonInteractionState={buttonInteractionState} 
                canInteractRef={canInteractRef} 
                fillUIActive = {fillUIActive} 
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

                            {/* Home Assistant Tank Heaters */}
                            <HeaterPanelComponent />
                        </div>
    
                        {/* Right Column */}
                        <div className="flex-1 flex flex-col gap-8">
                            {/* 6 Button Grid */}
                            <div className="bg-[#D9D9D9] border-[6px] border-black rounded-3xl p-5">
                                <div className="grid grid-cols-2 gap-[25px]">
                                <ButtonComponent buttonName="Solenoid Valve 1" currentState={valveData.SV1.actuated} actuationLock='LOCKED'/>
                                <ButtonComponent buttonName="Solenoid Valve 2" currentState={valveData.SV2.actuated} actuationLock='LOCKED'/>
                                <ButtonComponent buttonName="Ball Valve" currentState={valveData.BV.actuated} actuationLock='UNLOCKED'/>
                                <ButtonComponent buttonName="MAV" currentState={valveData.MAV.actuated} actuationLock='LOCKED'/> 
                                <ButtonComponent buttonName="Igniter" currentState={valveData.IG1.continuity && valveData.IG2.continuity} actuationLock='LOCKED'/>
                                <ButtonComponent buttonName="Quick Disconnect" currentState={valveData.QD.retracted} actuationLock='UNLOCKED'/>
                                </div>
                            </div>
    
                            

                            {/* Expand Button Panel */}
                            <ButtonPanelComponent />
                        </div>
                    </div>
                </div>
            </PropulsionContext.Provider>
        );
    }
    
    export default PropulsionPage;