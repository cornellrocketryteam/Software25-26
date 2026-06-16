import { useState } from 'react';
import ConfirmationOverlay from '../ConfirmationOverlayComponent';
import { usePropulsion } from '../../PropulsionPage';
import { useAppContext } from '../../App';
import { useButton } from '../ButtonComponent';
import type { ActuationTypeIdentifier } from '../../PropulsionPage';

// The single, designated launch button. Routed here by buttonName so there is
// exactly one launch control, and it is gated behind a two-step confirmation.
export default function InteractiveButtonComponent() {
  const [showConfirmation, setShowConfirmation] = useState(false);
  const [pendingAction, setPendingAction] = useState<ActuationTypeIdentifier | null>(null);
  // Launch confirmation progress: 0 = idle, 1 = first prompt, 2 = final prompt.
  const [launchStep, setLaunchStep] = useState(0);
  // Non-launch commands lock themselves locally after firing (one per button instance).
  // Only the launch button uses the shared, persisted hasLaunched from propulsion context.
  const { handleButtonClickRef, setButtonInteractionState, hasLaunched, setHasLaunched } = usePropulsion();
  const { wsRef } = useAppContext();
  const { buttonName, showState, currentState, label, stateLabel, actuationLock } = useButton();
  const [openLabel, closeLabel] = label;
  const openState: ActuationTypeIdentifier[] = ['OPEN', 'EXTEND']; // Define which actions correspond to "open" state
  const closedState: ActuationTypeIdentifier[] = ['CLOSE', 'RETRACT']; // Define which actions correspond to "close" state

  const sendLaunchCommand = () => {
    wsRef.current?.send(JSON.stringify({command: 'fsw_launch'}));
    console.log('LAUNCH command sent:', new Date().toISOString());
  };
  const sendResetFramCommand = () => {
    wsRef.current?.send(JSON.stringify({command: 'fsw_reset_fram'}));
    console.log('RESET FRAM command sent:', new Date().toISOString());
  };
  const sendWipeFlashCommand = () => {
    wsRef.current?.send(JSON.stringify({command: 'fsw_wipe_flash'}));
    console.log('WIPE FLASH command sent:', new Date().toISOString());
  };
  const sendRebootCommand = () => {
    wsRef.current?.send(JSON.stringify({command: 'fsw_reboot'}));
    console.log('REBOOT command sent:', new Date().toISOString());
  };
  const sendWipeRebootCommand = () => {
    wsRef.current?.send(JSON.stringify({command: 'fsw_wipe_fram_reboot'}));
    console.log('WIPE & FLASH command sent:', new Date().toISOString());
  };

  // Designated launch button: a single control with a two-step confirmation
  // (first confirm, then reprompt with "are you sure" for the final confirmation) before the command is sent.
  const isLaunch = buttonName === "Launch Button";
  // Launch uses the shared/persisted lock; every other command uses its own local lock.
  const isLocked = isLaunch ? hasLaunched : false; // We want the other 4 general commands to be re-usable, so they don't use the shared hasLaunched lock and instead just disable themselves locally after each use.

  if (isLaunch || buttonName === "Reset FRAM" || buttonName === "Wipe Flash" || buttonName === "Wipe & Reboot" || buttonName === "Reboot FSW") {
    return (
      <>
        <div className="bg-white border-[6px] border-black rounded-3xl p-4 flex flex-col items-center justify-center w-full overflow-hidden">
          <p className="font-inter text-2xl mb-2">{buttonName}</p>
          <button
            onClick={() => setLaunchStep(1)}
            disabled={isLocked}
            className={`${
              isLocked ? 'bg-[#9CA3AF] cursor-not-allowed' : (isLaunch ? 'bg-[#D63A1F] hover:opacity-90' : 'bg-[#5A87FF] hover:opacity-90')
            } border-[6px] border-black rounded-2xl w-full py-4 font-inter font-bold text-3xl text-white`}
          >
            {isLaunch ? (isLocked ? 'Launched' : 'Launch') : (isLocked ? 'Command Sent' : 'Send Command')}
          </button>
        </div>

        {launchStep === 1 && (
          <ConfirmationOverlay
            message="Confirm you want to send command?"
            onConfirm={() => setLaunchStep(2)}
            onCancel={() => setLaunchStep(0)}
          />
        )}
        {launchStep === 2 && (
          <ConfirmationOverlay
            message="FINAL CONFIRMATION: Are You Sure?"
            onConfirm={() => {

              switch (buttonName) {
                case "Reset FRAM":
                  sendResetFramCommand();
                  break;
                case "Wipe Flash":
                  sendWipeFlashCommand();
                  break;
                case "Wipe & Reboot":
                  sendWipeRebootCommand();
                  break;
                case "Reboot FSW":
                  sendRebootCommand();
                  break;
                case "Launch Button":
                  sendLaunchCommand();
                  break
              }
              //We want to be able to call the 4 general commands as often as we want...
              setLaunchStep(0);
            }}
            onCancel={() => setLaunchStep(0)}
          />
        )}
      </>
    );
  }

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
              <>
                <button
                  onClick={() => {
                    if(buttonName === "Solenoid Valve 1" || buttonName === "Solenoid Valve 2" || buttonName === "Ball Valve" || buttonName === "MAV"){
                      toggleAction('OPEN');
                    } else if (buttonName === "Quick Disconnect"){
                      toggleAction('RETRACT');
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
                      toggleAction('EXTEND');
                    }
                  }}
                  className={`${
                    !currentState && actuationLock === 'LOCKED'? 'bg-[#E27D7D]/50 cursor-not-allowed opacity-50' : 'bg-[#E27D7D]'
                  } border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                >
                  {closeLabel}
                </button>
              </>
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