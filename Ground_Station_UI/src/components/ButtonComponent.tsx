import { useEffect, useContext, createContext } from "react";
import { usePropulsion } from "../PropulsionPage";
import type { actuationLockType } from "../PropulsionPage";
import DisplayButtonComponent from "./subcomponents/DisplayButtonComponent";
import InteractiveButtonComponent from "./subcomponents/InteractiveButtonComponent";

interface ButtonComponentProps {
    actuationLock: actuationLockType;
    buttonName: string;
    transitioning?: boolean
    showState?: boolean;
    currentState: boolean;
}

interface ButtonContextType {
    buttonName: string;
    currentState: boolean;
    actuationLock: actuationLockType;
    transitioning: boolean;
    showState: boolean;
    label: [string, string];
    stateLabel: string;
}

const ButtonContext = createContext<ButtonContextType | null>(null);

export const useButton = () => {
    const context = useContext(ButtonContext);
    if (!context) throw new Error("useButton must be used within a ButtonComponent");
    return context;
};


const labelMap: { [key: string]: [string, string] } = {
    "Solenoid Valve 1": ["OPEN",      "CLOSE"],
    "Solenoid Valve 2": ["OPEN",      "CLOSE"],
    "Ball Valve":       ["OPEN",      "CLOSE"],
    "MAV":              ["OPEN",      "CLOSE"],
    "Igniter":           ["IGNITE",      "IGNITE"],
    "Quick Disconnect": ["EXTEND",      "RETRACT"],
  };
  
  const stateMap: { [key: string]: [string, string] } = {
    "Solenoid Valve 1": ["OPENED",    "CLOSED"],
    "Solenoid Valve 2": ["OPENED",    "CLOSED"],
    "Ball Valve":       ["OPENED",    "CLOSED"],
    "MAV":              ["OPENED",    "CLOSED"],
    "Igniter":           ["CONTINUITY","NO CONTINUITY"],
    "Quick Disconnect": ["RETRACTED", "EXTENDED"],
  };
  
  
  
export default function ButtonComponent({ buttonName, actuationLock, currentState = false, transitioning = false, showState = true }: ButtonComponentProps) {
    const {buttonInteractionState} = usePropulsion();
    const label = labelMap[buttonName] ?? ["ON", "OFF"];
    const stateLabels = stateMap[buttonName] ?? ["ON", "OFF"];
    const stateLabel = currentState ? stateLabels[0] : stateLabels[1];

    useEffect(() => {
        console.log(`${buttonName} changed to ${currentState ? label[0] : label[1]}`);
    }, [currentState]);


    const renderObject = () => {
        if(buttonInteractionState === 'DISABLED') {
            return (
                <DisplayButtonComponent />
            );
        } else if(buttonInteractionState === 'ENABLED') {
            return (
                <InteractiveButtonComponent />
            );
        }
    };

    return (
        <ButtonContext.Provider value={{ buttonName, currentState, actuationLock, transitioning, showState, label, stateLabel}}>
            <div>{renderObject()}</div>
        </ButtonContext.Provider>
    );
}