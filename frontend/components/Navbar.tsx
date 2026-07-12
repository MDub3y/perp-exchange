'use client';

import React, { useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Search, Sun, Moon, Menu, X } from 'lucide-react';
import { useExchangeStore, NavigationLink } from '@/store/useExchangeStore';
import Logo from './Logo';


export default function Navbar() {
    const { activeNavLink, setActiveNavLink, theme, toggleTheme } = useExchangeStore();
    const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

    const links: NavigationLink[] = ['Spot', 'Futures', 'Lend', 'Vault', 'Stocks', 'BP'];

    return (
        <nav className="max-w-7xl mx-auto w-full relative select-none">
            {/* DESKTOP FLOATING BLUR HEADER (Aceternity Layout Specification) */}
            <motion.div
                initial={{ y: -100, opacity: 0 }}
                animate={{ y: 0, opacity: 1 }}
                transition={{ type: 'spring', stiffness: 260, damping: 20 }}
                className="shadow-aceternity fixed inset-x-0 top-3 z-50 mx-auto hidden max-w-[calc(80rem-4rem)] items-center justify-between bg-white/80 px-6 py-2.5 backdrop-blur-md md:flex xl:rounded-2xl dark:bg-neutral-900/80 dark:shadow-[0px_2px_0px_0px_var(--color-neutral-800),0px_-2px_0px_0px_var(--color-neutral-800)] border border-neutral-200 dark:border-neutral-800"
            >
                {/* Brand Trigger Identity */}
                <a className="flex items-center gap-2 text-neutral-900 dark:text-[#f4f4f6] hover:opacity-80 transition-opacity" href="/">
                    <Logo className="w-5 h-6 text-[#E33E3F]" />
                    <span className="text-xl font-bold tracking-tight font-sans">
                        MXD <span className="text-[#9b9fae] font-normal">Helix</span>
                    </span>
                </a>

                {/* Dynamic Navigation Interactive Links */}
                <div className="flex items-center gap-6 lg:gap-8">
                    {links.map((link) => (
                        <button
                            key={link}
                            onClick={() => setActiveNavLink(link)}
                            className={`relative font-medium text-sm transition-colors duration-200 py-1 cursor-pointer ${activeNavLink === link
                                ? 'text-neutral-900 dark:text-[#f4f4f6]'
                                : 'text-gray-600 dark:text-gray-400 hover:text-neutral-900 dark:hover:text-neutral-200'
                                }`}
                        >
                            {link}
                            {activeNavLink === link && (
                                <motion.div
                                    layoutId="activeNavLine"
                                    className="absolute bottom-0 left-0 right-0 h-[2px] bg-[#E33E3F] rounded-full"
                                    transition={{ type: 'spring', stiffness: 380, damping: 30 }}
                                />
                            )}
                        </button>
                    ))}
                </div>

                {/* Action Callouts Panel Area */}
                <div className="flex items-center gap-3">
                    <button className="text-gray-600 dark:text-gray-400 hover:text-neutral-900 dark:hover:text-neutral-200 p-1.5 transition-colors">
                        <Search className="w-4 h-4" />
                    </button>

                    <button
                        onClick={toggleTheme}
                        className="relative flex cursor-pointer items-center justify-center rounded-xl p-2 text-gray-600 dark:text-gray-400 hover:text-[#f4f4f6] transition-colors"
                    >
                        {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
                    </button>

                    <a className="block rounded-xl px-5 py-1.5 text-center text-sm font-semibold transition duration-150 active:scale-[0.98] bg-neutral-900 text-white dark:bg-white dark:text-black border border-neutral-800 dark:border-neutral-200 hover:opacity-90" href="/trade/BTC_USD_PERP">
                        Start Trading
                    </a>
                </div>
            </motion.div>

            {/* MOBILE HEADER FRAME VIEWPORTS BOUNDS */}
            <div className="relative flex items-center justify-between p-4 md:hidden bg-[#0b0e11] border-b border-[#1c1e22] z-50 w-full">
                <a className="flex items-center gap-2" href="/">
                    <Logo className="w-5 h-6 text-[#E33E3F]" />
                    <span className="text-xl font-bold tracking-tight text-[#f4f4f6]">MXD Helix</span>
                </a>
                <button
                    onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                    className="shadow-aceternity flex size-8 items-center justify-center rounded-md bg-[#1c1e22] text-[#9b9fae] hover:text-[#f4f4f6]"
                    aria-label="Toggle menu"
                >
                    {mobileMenuOpen ? <X className="size-4" /> : <Menu className="size-4" />}
                </button>
            </div>

            {/* MOBILE OVERLAY NAVIGATION CONTAINER */}
            <AnimatePresence>
                {mobileMenuOpen && (
                    <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: 'auto' }}
                        exit={{ opacity: 0, height: 0 }}
                        className="absolute top-14 left-0 right-0 bg-[#121418] border-b border-[#1c1e22] flex flex-col p-4 gap-3 z-40 md:hidden"
                    >
                        {links.map((link) => (
                            <button
                                key={link}
                                onClick={() => {
                                    setActiveNavLink(link);
                                    setMobileMenuOpen(false);
                                }}
                                className={`text-left py-2 font-medium text-sm transition-colors ${activeNavLink === link ? 'text-[#f4f4f6] pl-2 border-l-2 border-[#E33E3F]' : 'text-[#9b9fae]'
                                    }`}
                            >
                                {link}
                            </button>
                        ))}
                    </motion.div>
                )}
            </AnimatePresence>
        </nav>
    );
}