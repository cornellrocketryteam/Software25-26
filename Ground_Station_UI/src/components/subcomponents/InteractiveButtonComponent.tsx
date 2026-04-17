import { useState } from 'react';
import ConfirmationOverlay from '../ConfirmationOverlayComponent';
import { usePropulsion } from '../../PropulsionPage';
import { useButton } from '../ButtonComponent';

export default function InteractiveButtonComponent() {
  const [showConfirmation, setShowConfirmation] = useState(false);
  const [pendingAction, setPendingAction] = useState<'open' | 'close' | null>(null);
  const { handleButtonClick } = usePropulsion();
  const { buttonName, isSpecial, showState, isOpen, label, stateLabel } = useButton();
  const [openLabel, closeLabel] = label;

  const toggleAction = (action: 'open' | 'close') => {
    if ((action === 'open' && isOpen) || (action === 'close' && !isOpen)) return;
    setPendingAction(action);
    setShowConfirmation(true);
  };

  const handleConfirm = () => {
    if (pendingAction === 'open' || pendingAction === 'close') {
      handleButtonClick(buttonName);
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
            {isSpecial && openLabel === closeLabel ? (
              <button
                onClick={() => toggleAction('open')}
                className="bg-[#555555] border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white"
              >
                {openLabel}
              </button>
            ) : (
              <>
                <button
                  onClick={() => toggleAction('open')}
                  className={`${
                    isOpen ? 'bg-[#ADC7AC]/50 cursor-not-allowed opacity-50' : 'bg-[#ADC7AC]'
                  } border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                >
                  {openLabel}
                </button>
                <button
                  onClick={() => toggleAction('close')}
                  className={`${
                    !isOpen ? 'bg-[#E27D7D]/50 cursor-not-allowed opacity-50' : 'bg-[#E27D7D]'
                  } border-[6px] border-black rounded-2xl w-full py-3 font-inter font-bold text-2xl text-white`}
                >
                  {closeLabel}
                </button>
              </>
            )}
          </div>

          {showState && (
            <div className={`${isOpen ? 'bg-[#ADC7AC]' : 'bg-[#E27D7D]'} border-[6px] border-black rounded-2xl px-6 py-4 flex flex-col items-center justify-center min-w-[120px]`}>
              <p className="font-inter font-bold text-sm text-white mb-2">
                State: {stateLabel}
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