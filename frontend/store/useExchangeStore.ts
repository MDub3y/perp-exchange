import { create } from 'zustand';

export type NavigationLink = 'Spot' | 'Futures' | 'Lend' | 'Vault' | 'Stocks' | 'BP';
export type MarketCategory = 'Spot' | 'Futures' | 'Stocks' | 'Lend';

interface ExchangeUIStore {
    activeNavLink: NavigationLink;
    activeCategory: MarketCategory;
    theme: 'dark' | 'light';
    setActiveNavLink: (link: NavigationLink) => void;
    setActiveCategory: (cat: MarketCategory) => void;
    toggleTheme: () => void;
}

export const useExchangeStore = create<ExchangeUIStore>((set) => ({
    activeNavLink: 'Futures',
    activeCategory: 'Futures',
    theme: 'dark',
    setActiveNavLink: (activeNavLink) => set({ activeNavLink }),
    setActiveCategory: (activeCategory) => set({ activeCategory }),
    toggleTheme: () => set((state) => ({ theme: state.theme === 'dark' ? 'light' : 'dark' })),
}));