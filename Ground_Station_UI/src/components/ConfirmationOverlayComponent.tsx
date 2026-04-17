interface ConfirmationOverlayProps {
    message: string;
    onConfirm: () => void;
    onCancel: () => void;
  }
  
  export default function ConfirmationOverlay({ message, onConfirm, onCancel }: ConfirmationOverlayProps) {
    return (
      <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        <div className="bg-[#929292] border-[6px] border-black rounded-3xl px-12 py-8 flex items-center gap-60 w-[80vw]">          {/* Left side - Are You Sure with warning icons */}
          <div className="flex items-center gap-4">
            <h2 className="font-inter font-bold text-[54px]">{message}</h2>
          </div>
  
          {/* Right side - Yes/No buttons */}
          <div className="flex gap-6">
            <button
              onClick={onConfirm}
              className="bg-[#E27D7D] border-[6px] border-black rounded-3xl px-16 py-4 font-inter font-bold text-[36px] text-white hover:opacity-90"
            >
              Yes
            </button>
            <button
              onClick={onCancel}
              className="bg-[#1A1A1A] border-[6px] border-black rounded-3xl px-16 py-4 font-inter font-bold text-[36px] text-white hover:opacity-90"
            >
              No
            </button>
          </div>
        </div>
      </div>
    );
  }