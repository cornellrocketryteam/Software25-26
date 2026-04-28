import { useState } from 'react';
import ConfirmationOverlay from '../ConfirmationOverlayComponent';
import { usePropulsion } from '../../PropulsionPage';
import { useButton } from '../ButtonComponent';
import type { ActuationTypeIdentifier } from '../../PropulsionPage';

export default function InteractiveButtonComponent() {
  const [showConfirmation, setShowConfirmation] = useState(false);
  const [pendingAction, setPendingAction] = useState<ActuationTypeIdentifier | null>(null);
  const { handleButtonClickRef } = usePropulsion();
  const { buttonName, showState, currentState, label, stateLabel, actuationLock } = useButton();
  const [openLabel, closeLabel] = label;
  const openState: ActuationTypeIdentifier[] = ['OPEN', 'EXTEND', 'IGNITE']; // Define which actions correspond to "open" state
  const closedState: ActuationTypeIdentifier[] = ['CLOSE', 'RETRACT']; // Define which actions correspond to "close" state

  const toggleAction = (action: ActuationTypeIdentifier) => {

    if (((openState.includes(action) && currentState) || (closedState.includes(action) && !currentState)) 
      && actuationLock === 'LOCKED') {
      return;
    } 
    setPendingAction(action);
    setShowConfirmation(true);
  };

  const handleConfirm = () => {
    if (pendingAction !== null) { //Pending Action has some action stored
      handleButtonClickRef.current(buttonName, pendingAction);
    }
    setShowConfirmation(false);
    setPendingAction(null);
  };

  const handleCancel = () => {
    setShowConfirmation(false);
    setPendingAction(null);
  };

  return (
    <>
      <div className="bg-white border-[6px] border-black rounded-3xl p-4 flex flex-col items-center justify-center w-full overflow-hidden">
        <p className="font-inter text-2xl mb-2">{buttonName}</p>
        
        <div className="flex gap-2">
        <div className="flex flex-col gap-2 min-w-0 w-full">
            {openLabel === closeLabel ? ( // Special case for buttons like "Igniter" and "Launch" where both states have the same label
              <button
                onClick={() => { //This is just a one time button click
                  if(buttonName === "Igniter"){
                    toggleAction('IGNITE'); // For igniter, we can still use OPEN/CLOSE as the action identifiers even though the labels are the same
                  } else if (buttonName === "LAUNCH"){
                    //run action for launch, which will be updated
                    console.log("LAUNCH BUTTON PRESSED - RUN LAUNCH SEQUENCE ACTION HERE");
                  }
                }}
                className="bg-[#555555] border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white"
              >
                {openLabel}
              </button>
            ) : (
              <>
                <button
                  onClick={() => {
                    if(buttonName === "Solenoid Valve 1" || buttonName === "Solenoid Valve 2" || buttonName === "Ball Valve" || buttonName === "MAV"){
                      toggleAction('OPEN');
                    } else if (buttonName === "Quick Disconnect"){
                      toggleAction('EXTEND');
                    }
                  }}
                  className={`${
                    currentState && actuationLock === 'LOCKED' ? 'bg-[#ADC7AC]/50 cursor-not-allowed opacity-50' : 'bg-[#ADC7AC]'
                  } border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                >
                  {openLabel}
                </button>
                <button
                  onClick={() => {
                    if(buttonName === "Solenoid Valve 1" || buttonName === "Solenoid Valve 2" || buttonName === "Ball Valve" || buttonName === "MAV"){
                      toggleAction('CLOSE');
                    } else if (buttonName === "Quick Disconnect"){
                      toggleAction('RETRACT');
                    }
                  }}
                  className={`${
                    !currentState && actuationLock === 'LOCKED'? 'bg-[#E27D7D]/50 cursor-not-allowed opacity-50' : 'bg-[#E27D7D]'
                  } border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                >
                  {closeLabel}
                </button>
              </>
            )}
          </div>

          {showState && ( // Only show state indicator if showState is true - allows flexibility for buttons that don't need a state display like the backup launch button
            <div className={`${currentState ? 'bg-[#ADC7AC]' : 'bg-[#E27D7D]'} border-[6px] border-black rounded-2xl px-6 py-4 flex flex-col items-center justify-center min-w-[120px]`}>
              <p className="font-inter font-bold text-sm text-white mb-2">
                State: {stateLabel}
              </p>
              <div className="w-12 h-12 border-4 border-black rounded-full flex items-center justify-center">
                {currentState ? (
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
          )}
        </div>
      </div>

      {showConfirmation && (
        <ConfirmationOverlay
          message="Are You Sure"
          onConfirm={handleConfirm}
          onCancel={handleCancel}
        />
      )}
    </>
  );
}