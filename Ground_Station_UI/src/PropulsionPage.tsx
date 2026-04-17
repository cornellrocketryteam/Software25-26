import Header from "./components/HeaderComponent";
import ButtonComponent from "./components/ButtonComponent";
import VentButtonComponent from "./components/VentButtonComponent";
import FillButtonComponent from "./components/FillButtonComponent";
import ButtonPanelComponent from "./components/ButtonPanelComponent";
import {useEffect, useRef, useState} from "react";
import { createContext, useContext} from "react";


export type FillState = 'INITIAL' | 'INTERVENE' | 'SAFE_PROCEDURE' | 'STOP_FILL';
export type FlightMode = "....."| 'STANDBY';


type ValveData = {
    SV1: {"actuated": boolean, "continuity": boolean};
    SV2: {"actuated": boolean, "continuity": boolean};
    MAV: {"actuated": boolean, "angle": number, "pulseWidth": number};
    BV:  {"actuated": boolean, "state": string};
    IG1: {"continuity": boolean};
    IG2: {"continuity": boolean};
}

type PropulsionContextType = {
    handleButtonClick: (valveName: string) => void;
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
    currFlightMode: FlightMode;
    adcDataRef: React.RefObject<AdcDataMessage[]>;
    telemetryDataRef: React.RefObject<FswTelemetryMessage[]>;
    isVentingRef: React.RefObject<boolean>;
    isFillingRef: React.RefObject<boolean>;
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

export const usePropulsion = () => { //Custom hook to access propulsion context, will throw an error if used outside of the PropulsionPage component which provides the context
    const context = useContext(PropulsionContext);
    if (!context) throw new Error("usePropulsion must be used inside PropulsionPage (i.e., this useContext hook can only be called by components that are children of the PropulsionContext.Provider in the component tree)");
    return context;
};

export function PropulsionPage() {
    const [fillState, setFillState] = useState<FillState>('INITIAL');
    const [currFlightMode, setCurrFlightMode] = useState<FlightMode>('.....');
    const [ventSeconds, setVentSeconds] = useState(0);
    const [confirmedVentSeconds, setConfirmedVentSeconds] = useState(0);
    const [thresholdPressure, setThresholdPressure] = useState(600); 
    const [ventUIActive, setVentUIActive] = useState(false); // State to control whether the vent UI is active and visible
    const [fillUIActive, setFillUIActive] = useState(false); // State to control whether the fill UI is active and visible


    
    const uri = "ws://192.168.8.167:9000";
    //const uri = "ws://localhost:9000"; // Replace with the WebSocket server URI
    const wsRef = useRef<WebSocket | null>(null); // Use useRef to store the WebSocket instance
    const pendingActionRef = useRef<ValveKey | null>(null); // Use useRef to store the pending action for confirmation
    const adcDataRef = useRef<AdcDataMessage[]>([]); // Use useRef to store the latest ADC data received from the server
    const umbilicalDataRef = useRef<FswTelemetryMessage[]>([]); // Use useRef to store the latest telemetry data received from the server
    const manualVentRef = useRef(false); // We can use a ref for this since we don't necessarily need to trigger a re-render when this value changes
    const isVentingRef = useRef(false); // Track whether a vent is currently in progress so the interval doesn't stack multiple vent cycles while the 1-second SV2 timeout is running.
    const isFillingRef = useRef(false); // Mirror isFilling into a ref so the interval can check it synchronously without capturing a stale closure value.

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


    //Solenoid Valve Commands: Get the information of the valve states on mount (so call in useEffect) so I can set my initial state of the valve buttons to 
    //match the actual state of the valves, and then also use these commands to get updated valve states after sending any command that would change the valve states.
    const getSVstate1 = {"command": "get_valve_state", "valve": "SV1"};
    const getSVstate2 = {"command": "get_valve_state", "valve": "SV2"};
    const actuateSV1Open = {"command": "actuate_valve", "valve": "SV1", "open": true};
    const actuateSV2Open = {"command": "actuate_valve", "valve": "SV2", "open": true};
    const actuateSV1Close = {"command": "actuate_valve", "valve": "SV1", "open": false};
    const actuateSV2Close = {"command": "actuate_valve", "valve": "SV2", "open": false};

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


    const buttondelay = 250; // Example delay in milliseconds between commands, can adjust as needed based on testing and how quickly the server can process commands and send responses

    //COMMAND DELAY FUNCTION
    const sendCommandWithDelay = (command: any, delay: number) => {
        setTimeout(() => {

            //If we are calling a command that gets the values of a valve, we want to set that as our pending action so that when we get the response back from the server, we know which valve's data we're getting back and can update our state accordingly
            if(queryCommands.includes(command.command)){ 
                pendingActionRef.current = command.valve; // Set the pending action to the valve we're getting the state of
            }

            //If we are calling a command that actuates a valve, we want to clear any pending action since we know that any information we get back from the server after sending this command will be outdated and not relevant to our current state
            else if(actuationCommands.includes(command.command)){
                pendingActionRef.current = null; // Clear the pending action since we're changing the state and any information we get back will be outdated
            }
            wsRef.current?.send(JSON.stringify(command)); //Send the command passed to the function through the WebSocket connection after the specified delay
        }, delay);
    };
    
    //This function is called when a valve button is clicked, and it determines which command to send based on the name of the button and the current state of the corresponding valve in our state. 
    //It also sets the pending action for query commands so that we can update our state with the response from the server when we get it back.
    const handleButtonClick = (ValveName: string) => { 

        const commandMap : {[key: string]: string} = { //---> map names of the buttons to shorthand identifiers
            "Solenoid Valve 1": "SV1",
            "Solenoid Valve 2": "SV2",
            "Ball Valve": "BV",
            "Igniter": "Igniter",
            "MAV": "MAV",
            "Quick Disconnect": "QD"
        }

        const valveIdentifier = commandMap[ValveName]; //<= This will map the button name to the corresponding valve identifier used in the command

        switch (valveIdentifier) { //Handles the logic for the button click based off the last saved state of the button
            case "SV1":
                valveData.SV1.actuated ? sendCommandWithDelay(actuateSV1Close, buttondelay) : sendCommandWithDelay(actuateSV1Open, buttondelay); //Send appropriate command to toggle based on toState
                sendCommandWithDelay(getSVstate1, buttondelay + 50); //Get updated state of SV1 after actuating it to confirm it changed and to update our state with the new information
                console.log(`Toggling Solenoid Valve 1 to ${!valveData.SV1.actuated ? 'OPEN' : 'CLOSED'}`); //We should've changed the value to the opposite of the current state with our command, so we log that we're toggling it to the opposite of the current state
                break;
            case "SV2":
                valveData.SV2.actuated ? sendCommandWithDelay(actuateSV2Close, buttondelay) : sendCommandWithDelay(actuateSV2Open, buttondelay); //Send appropriate command to toggle based on toState
                sendCommandWithDelay(getSVstate2, buttondelay + 50); //Get updated state of SV2 after actuating it to confirm it changed and to update our state with the new information
                console.log(`Toggling Solenoid Valve 2 to ${!valveData.SV2.actuated ? 'OPEN' : 'CLOSED'}`); //We should've changed the value to the opposite of the current state with our command, so we log that we're toggling it to the opposite of the current state
                break;
            case "BV": //Query issues
                valveData.BV.actuated ? sendCommandWithDelay(actuateBallValveClose, buttondelay) : sendCommandWithDelay(actuateBallValveOpen, buttondelay); //Send appropriate command to toggle based on toState
                //Manually update state since we have no way of querrying the actual state of the ball valve, so we have to assume that our command works correctly and update our state accordingly
                valveData.BV.actuated ? setValveData(prevState => ({...prevState, BV: {"actuated": false, "state": "high"}})) : setValveData(prevState => ({...prevState, BV: {"actuated": true, "state": "low"}})); 
                console.log(`Toggling Ball Valve to ${!valveData.BV.actuated ? 'OPEN' : 'CLOSED'}`); //We should've changed the value to the opposite of the current state with our command, so we log that we're toggling it to the opposite of the current state
                break;
            case "MAV": 
                if(valveData.MAV.actuated) { //If the Mav is actuated then we want to close it
                    sendCommandWithDelay(actuateMavClose, buttondelay); //Send `Close` Command for Mav Valve
                } else if(!valveData.MAV.actuated) { //If the Mav is not actuated then we want to open it
                    sendCommandWithDelay(actuateMavOpen, buttondelay); //Send `Open` Command for Mav Valve
                }
                
                // We no longer query MAV state directly. It updates via FSW Telemetry stream automatically.
                console.log(`Toggling MAV to ${!valveData.MAV.actuated ? 'OPEN' : 'CLOSED'}`); //We should've changed the value to the opposite of the current state with our command, so we log that we're toggling it to the opposite of the current state
                break;
            case "QD":
                valveData.QD.retracted ? sendCommandWithDelay(actuateQDClose, buttondelay) : sendCommandWithDelay(actuateQDOpen, buttondelay); //Send appropriate command to toggle based on toState
                break;
            case "Igniter":
                if (valveData.IG1.continuity && valveData.IG2.continuity) { //Only send ignite command if we have continuity for both igniters, which means we have the ability to ignite    
                    sendCommandWithDelay(igniteCommand, buttondelay); //Send ignite command
                }
                break;
            default:
                console.error("Unknown valve name:", ValveName);
        }
    };
    
    //useEffect hook sets up connection to fill station server on mount of propulsion page, and also sets up listeners for messages, connection closures, and errors, 
    //and then cleans up the connection on unmount of the page. The connection closure listener also attempts to reconnect after a delay if the connection is lost, and 
    //the error listener ensures that any errors also trigger a closure and reconnection attempt.
    useEffect(() => {
        let reconnectTimeout: ReturnType<typeof setTimeout>;

        const connect = () => {
            //
            let heartbeatInterval: ReturnType<typeof setInterval>; 
            let pollingInterval: ReturnType<typeof setInterval>;

            // This is where you would set up your WebSocket connection and listeners
            wsRef.current = new WebSocket(uri);

            if(wsRef.current === null){
                console.log("Something is bugging out with the WebSocket connection, check if the server is running and the URI is correct");
                return;
            }

            wsRef.current.onopen = () => {
                console.log("WebSocket connection established.");

                sendCommandWithDelay(getSVstate1, 0); // Get initial state of SV1 immediately on connection
                sendCommandWithDelay(getSVstate2, 50); // Get initial state of SV2 immediately on connection
                sendCommandWithDelay(getIgniterContinuity1, 50); // Get initial continuity status of igniter 1 immediately on connection, just for log as mentioned previously
                sendCommandWithDelay(getIgniterContinuity2, 50); // Get initial continuity status of igniter 2 immediately on connection, just for log as mentioned previously
                sendCommandWithDelay(heartbeatCommand, 50); // Send initial heartbeat immediately on connection

                //Delays chosen arbitrarily 
                sendCommandWithDelay(actuateSV2Open, 150); //Normally on, so turn on after delay to ensure it gets the correct initial state before changing it
                sendCommandWithDelay(actuateMavClose, 150); //Normally closed, so close after delay to ensure it gets the correct initial state before changing it

                sendCommandWithDelay(getSVstate2, 200); //Get updated state of SV2 after actuating it to confirm it changed and to update our state with the new information
                sendCommandWithDelay(startADCStream, 333);
                sendCommandWithDelay(startFSWStream, 333);
                //  ^
                //  |
                //  |
                //  |
                //Helps with a lot, but for simply viewing our updates to some of our code, I do not want this thing cluttering my 
                //console with ADC data, so I will comment it out for now, but this is where I would send the command to start the ADC 
                //stream to start receiving that data from the server
                console.log("Command batch finished");

                //Start heartbeat loop to maintain connection with live fill station server
                heartbeatInterval = setInterval(() => {
                    if (wsRef.current?.readyState === WebSocket.OPEN) {
                        wsRef.current.send(JSON.stringify(heartbeatCommand));
                    }
                }, 5000);

                //Start polling loop to get updated valve states every 3 seconds, which will help ensure that our state stays up to date 
                //with the actual state of the system even if we miss some responses from the server or if there are changes to the system 
                //state that we don't directly trigger with a command (e.g., if someone manually changes the state of a valve without using our interface) <- This is incredibly important
                pollingInterval = setInterval(() => {
                    if (wsRef.current?.readyState === WebSocket.OPEN) {
                        sendCommandWithDelay(getSVstate1, 0);
                        sendCommandWithDelay(getSVstate2, 50);
                        sendCommandWithDelay(getIgniterContinuity1, 150);
                        sendCommandWithDelay(getIgniterContinuity2, 200);
                    }
                }, 3000);
            };

            wsRef.current.onmessage = (event) => {
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
                        if(umbilicalDataRef.current.length > 500) { //Limit the size of the ADC data array to prevent memory issues, adjust as needed based on how much data you want to keep track of
                            umbilicalDataRef.current.shift(); //Remove the oldest entry when we exceed the limit
                        }


                        const lastPressure = umbilicalDataRef.current.at(-1)?.telemetry.pt4 ?? 0;
    
                        if (isFillingRef.current && !isVentingRef.current) {
                            data.telemetry.pt4 = lastPressure + (Math.random() * 4 + 1); // Random increase between 1-5 during fill
                        } else if (isVentingRef.current) {
                            data.telemetry.pt4 = Math.max(0, lastPressure - (Math.random() * 15 + 10)); // Random decrease between 10-25 during vent
                        } else {
                            data.telemetry.pt4 = lastPressure; // Hold when idle
                        }

                        console.log("Pressure:", new Date().toISOString(), "PSI:", data.telemetry.pt4);

                        setCurrFlightMode(data.flight_mode as FlightMode); //Update our current flight mode state with the latest flight mode from the telemetry data, which we can use to display on the page or for any logic we want to implement based on the flight mode in the future
                        umbilicalDataRef.current.push(data); //Store the latest ADC data in the ref, which we can use for displaying on the page or for any logic we want to implement based on the ADC data in the future
                        
                        // Update MAV state based on telemetry
                        if (data.telemetry && data.telemetry.mav_open !== undefined) {
                            setValveData(prevState => ({
                                ...prevState,
                                MAV: { "actuated": data.telemetry.mav_open, "angle": 0, "pulseWidth": 0 }
                            }));
                        }
                        break;

                    case "igniter_continuity":
                        if (data.id === 1) {
                            setValveData(prevState => ({
                                ...prevState, //Spread the previous state to keep other valves' data unchanged
                                IG1: { "continuity": data.continuity }
                                //Adjust as needed based on actual response data and what information we want to track
                            }));
                        }
                        else if (data.id === 2) {
                            setValveData(prevState => ({
                                ...prevState, //Spread the previous state to keep other valves' data unchanged
                                IG2: { "continuity": data.continuity }
                                //Adjust as needed based on actual response data and what information we want to track
                            }));
                        }

                    break;
                    
                    case "valve_state":
                        //Check pending action to see which valve this response is for and update state accordingly
                        if (pendingActionRef.current === "SV1" || data.valve === "SV1") {
                            setValveData(prevState => ({
                                ...prevState, //Spread the previous state to keep other valves' data unchanged
                                SV1: { "actuated": data.open, "continuity": data.continuity }
                                //Adjust as needed based on actual response data and what information we want to track
                            }));
                        }
                        else if (pendingActionRef.current === "SV2" || data.valve === "SV2") {
                            setValveData(prevState => ({
                                ...prevState, //Spread the previous state to keep other valves' data unchanged
                                SV2: { "actuated": data.open, "venting": false, "continuity": data.continuity }
                                //Adjust as needed based on actual response data and what information we want to track
                            }));
                        }
                        break;
                }
            };

            wsRef.current.onclose = () => {
                clearInterval(heartbeatInterval);
                clearInterval(pollingInterval);
                console.log("WebSocket connection closed. Attempting to reconnect in 3 seconds...");
                reconnectTimeout = setTimeout(connect, 3000);
            };

            wsRef.current.onerror = (error) => {
                clearInterval(heartbeatInterval);
                clearInterval(pollingInterval);
                console.error("WebSocket error:", error);
                wsRef.current?.close(); // Ensure closure to trigger onclose and reconnect
            };
        };

        connect();

        //Return function to clean up the WebSocket connection when the component unmounts
        return () => {
            clearTimeout(reconnectTimeout);

            if (wsRef.current) {
                // Prevent onclose from triggering a reconnect when deliberately unmounting
                wsRef.current.onclose = null;
                wsRef.current.onerror = null;

                if (wsRef.current.readyState === WebSocket.OPEN) {
                    //Shut OFF no delay
                    wsRef.current.send(JSON.stringify(stopADCStream));
                    wsRef.current.send(JSON.stringify(stopFSWStream));
                }
                wsRef.current.close(); // Close the WebSocket connection when the component unmounts
                console.log("WebSocket connection cleanup."); //Log Cleanup
            }
        };
    }, []);
             
    return (
        <PropulsionContext.Provider value={{fillUIActive, setFillUIActive, ventUIActive, setVentUIActive, isVentingRef, isFillingRef, manualVentRef, handleButtonClick, fillState, setFillState, thresholdPressure, setThresholdPressure, ventSeconds, setVentSeconds, confirmedVentSeconds, setConfirmedVentSeconds, valveData, currFlightMode, adcDataRef: adcDataRef, telemetryDataRef: umbilicalDataRef}}>
            <div className={`min-h-screen bg-white ${ventUIActive ? 'cursor-wait' : ''}`}>
                {/* Header */}       
                <Header pageTitle="Propulsion Page" />
            
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
                            <ButtonComponent buttonName="Solenoid Valve 1" currentState={valveData.SV1.actuated}/>
                            <ButtonComponent buttonName="Solenoid Valve 2" currentState={valveData.SV2.actuated}/>
                            <ButtonComponent buttonName="Ball Valve" currentState={valveData.BV.actuated}/>
                            <ButtonComponent buttonName="MAV" currentState={valveData.MAV.actuated} /> 
                            <ButtonComponent buttonName="Ignite" currentState={valveData.IG1.continuity && valveData.IG2.continuity} isSpecial = {true}/>
                            <ButtonComponent buttonName="Quick Disconnect" currentState={valveData.QD.retracted} isSpecial = {true}/>
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