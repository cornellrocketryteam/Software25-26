import { useEffect, useState, useContext, createContext } from "react";
import { usePropulsion } from "../PropulsionPage";
import DisplayButtonComponent from "./subcomponents/DisplayButtonComponent";
import InteractiveButtonComponent from "./subcomponents/InteractiveButtonComponent";

interface ButtonComponentProps {
    buttonName: string;
    transitioning?: boolean
    showState?: boolean;
    isSpecial?: boolean;
    currentState: boolean;
}

interface ButtonContextType {
    buttonName: string;
    isOpen: boolean;
    isSpecial: boolean;
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
    "Ignite":           ["IGNITE",      "IGNITE"],
    "Quick Disconnect": ["EXTEND",      "RETRACT"],
  };
  
  const stateMap: { [key: string]: [string, string] } = {
    "Solenoid Valve 1": ["OPENED",    "CLOSED"],
    "Solenoid Valve 2": ["OPENED",    "CLOSED"],
    "Ball Valve":       ["OPENED",    "CLOSED"],
    "MAV":              ["OPENED",    "CLOSED"],
    "Ignite":           ["CONTINUITY","NO CONTINUITY"],
    "Quick Disconnect": ["EXTENDED", "RETRACTED"],
  };
  
  
  
export default function ButtonComponent({ buttonName, isSpecial = false, currentState = false, transitioning = false, showState = true }: ButtonComponentProps) {
    const [isOpen, setIsOpen] = useState(currentState);
    const {fillState} = usePropulsion();

    const label = labelMap[buttonName] ?? ["ON", "OFF"];
    const stateLabels = stateMap[buttonName] ?? ["ON", "OFF"];
    const stateLabel = currentState ? stateLabels[0] : stateLabels[1];

    useEffect(() => {
        setIsOpen(currentState);
        console.log(`${buttonName} changed to ${currentState ? label[0] : label[1]}`);
    }, [currentState]);


    const renderObject = () => {
        if (fillState === 'INITIAL' || fillState === 'INTERVENE') {
            return (
                <DisplayButtonComponent />
            );
        } else if (fillState === 'SAFE_PROCEDURE' || fillState === 'STOP_FILL') {
            return (
                <InteractiveButtonComponent />
            );
        }
    };

    return (
        <ButtonContext.Provider value={{ buttonName, isOpen, isSpecial, transitioning, showState, label, stateLabel}}>
            <div>{renderObject()}</div>
        </ButtonContext.Provider>
    );
}