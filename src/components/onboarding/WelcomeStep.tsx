import sunLogo from '../../assets/sun-logo.jpg';

interface WelcomeStepProps {
  isAnimating: boolean;
  onNext: () => void;
}

export function WelcomeStep({ isAnimating, onNext }: WelcomeStepProps) {
  return (
    <div className={`text-center transition-all duration-500 ${isAnimating ? 'opacity-0 scale-95' : 'opacity-100 scale-100'}`}>
      <div className="w-32 h-32 mx-auto mb-6 rounded-full overflow-hidden shadow-2xl ring-4 ring-orange-500/20">
        <img src={sunLogo} alt="4DA" className="w-full h-full object-cover" />
      </div>
      <h1 className="text-4xl font-semibold text-white mb-3">Welcome to 4DA</h1>
      <p className="text-xl text-orange-400 mb-2 font-medium">All signal. No feed.</p>
      <p className="text-gray-500 mb-8 max-w-md mx-auto">
        4DA learns what you care about and surfaces relevant content from across the internet - before you know you need it.
      </p>
      <div className="space-y-3 text-left bg-[#141414] p-5 rounded-lg mb-8 max-w-md mx-auto">
        <ul className="text-gray-400 space-y-3">
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-green-500/20 rounded-lg flex items-center justify-center">
              <span className="text-green-400">&#x1f512;</span>
            </span>
            <div>
              <strong className="text-white block">100% Private</strong>
              <span className="text-sm">All processing happens locally</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span className="text-orange-400">&#x26a1;</span>
            </span>
            <div>
              <strong className="text-white block">Autonomous</strong>
              <span className="text-sm">Self-discovering, learns from you</span>
            </div>
          </li>
          <li className="flex items-start gap-3">
            <span className="flex-shrink-0 w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
              <span className="text-blue-400">&#x1f511;</span>
            </span>
            <div>
              <strong className="text-white block">BYOK</strong>
              <span className="text-sm">Your API keys, you control costs</span>
            </div>
          </li>
        </ul>
      </div>
      <button
        onClick={onNext}
        className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
      >
        Get Started &rarr;
      </button>
      <p className="text-xs text-gray-600 mt-4">Takes about 2 minutes to set up</p>
    </div>
  );
}
