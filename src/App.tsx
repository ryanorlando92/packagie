import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { confirm, message } from '@tauri-apps/plugin-dialog';
import { LazyStore } from '@tauri-apps/plugin-store';

const store = new LazyStore('settings.json');

const SecureStore = {
    async getKey(hardwareSeed: string) {
        const enc = new TextEncoder();
        const keyMaterial = await crypto.subtle.importKey(
            "raw", enc.encode(hardwareSeed), { name: "PBKDF2" }, false, ["deriveKey"]
        );
        return await crypto.subtle.deriveKey(
            { name: "PBKDF2", salt: enc.encode("packagie_salt"), iterations: 100000, hash: "SHA-256" },
            keyMaterial, { name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]
        );
    },
    async encrypt(plainText: string, hardwareSeed: string) {
        if (!plainText) return "";
        const key = await this.getKey(hardwareSeed);
        const iv = crypto.getRandomValues(new Uint8Array(12));
        const enc = new TextEncoder();
        const encrypted = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, enc.encode(plainText));
        const combined = new Uint8Array(iv.length + encrypted.byteLength);
        combined.set(iv);
        combined.set(new Uint8Array(encrypted), iv.length);
        return btoa(String.fromCharCode(...combined));
    },
    async decrypt(cipherText: string, hardwareSeed: string) {
        if (!cipherText) return "";
        try {
            const key = await this.getKey(hardwareSeed);
            const combined = new Uint8Array(atob(cipherText).split('').map(c => c.charCodeAt(0)));
            const iv = combined.slice(0, 12);
            const data = combined.slice(12);
            const decrypted = await crypto.subtle.decrypt({ name: "AES-GCM", iv }, key, data);
            return new TextDecoder().decode(decrypted);
        } catch (e) {
            console.warn("Could not decrypt password.");
            return ""; 
        }
    }
};

export default function App() {
    const hasAttemptedLogin = useRef(false);
    const [showSettings, setShowSettings] = useState(false);
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [filePath, setFilePath] = useState('');
    const [status, setStatus] = useState('Ready');
    const [progress, setProgress] = useState(0);
    const [isProcessing, setIsProcessing] = useState(false);
    const [isBh, set_isBh] = useState(false);

    useEffect(() => {
        if (!hasAttemptedLogin.current) {
            hasAttemptedLogin.current = true;
            triggerAutoLogin();
        }
    }, []);

    const triggerAutoLogin = async () => {
        try {
            const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
            sleep(4000);
            const savedUsername = await store.get<string>('username');
            const savedEncryptedPass = await store.get<string>('password');

            if (savedUsername && savedEncryptedPass) {
                const hardwareKey = await invoke<string>('get_hardware_key');
                const decryptedPass = await SecureStore.decrypt(savedEncryptedPass, hardwareKey);
                
                if (decryptedPass) {
                    console.log("Credentials found. Triggering auto-login...");
                    await invoke('auto_login', { username: savedUsername, pass: decryptedPass });
                }
            }
        } catch (error) {
            console.error("Auto-login sequence failed:", error);
        }
    };

    const openSettings = async () => {
        try {
            const savedUser = await store.get<string>('username');
            const savedEncryptedPass = await store.get<string>('password');

            if (savedUser) setUsername(savedUser);
            
            if (savedEncryptedPass) {
                const hardwareKey = await invoke<string>('get_hardware_key');
                const decryptedPass = await SecureStore.decrypt(savedEncryptedPass, hardwareKey);
                if (decryptedPass) setPassword(decryptedPass);
            }
            
            setShowSettings(true);
        } catch (error) {
            console.error("Failed to load settings into UI:", error);
            setShowSettings(true); 
        }
    };

    const saveSettings = async () => {
        const hardwareKey = await invoke<string>('get_hardware_key');
        
        await store.set('username', username);
        
        if (password) {
            const encrypted = await SecureStore.encrypt(password, hardwareKey);
            await store.set('password', encrypted);
        }
        
        await store.save(); 
        
        await message("Your credentials have been securely encrypted and saved.", {
            title: 'Settings Saved',
            kind: 'info'
        });

        setShowSettings(false);
    };

    useEffect(() => {
        const unlisten = listen('import-progress', (event: any) => {
        setStatus(event.payload.message);
        if (event.payload.total > 0) {
            setProgress((event.payload.current / event.payload.total) * 100);
        }
        if (event.payload.message === "Import Complete!") {
            setIsProcessing(false);
        }
        });
        return () => { unlisten.then(f => f()); };
    }, []);

    const handleSelectFile = async () => {
        const selected = await open({ multiple: false, filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
        if (selected) setFilePath(selected as string);
    };

    const handleStart = async () => {
        if (!filePath) return;
        setIsProcessing(true);
        try {
        await invoke('start_import', { filePath, isBh });
        } catch (error) {
        setStatus(`Error: ${error}`);
        setIsProcessing(false);
        }
    };

    return (
        <main className="container">            
        <div style={{ fontFamily: 'Verdana, sans-serif' }}>
        <h2
            style={{ margin: '0px' }}
        >Dutchie Package Importer</h2>
        <div style={{ display: 'flex', gap: '10px', margin: '20px' }}>
            <input 
            type="text" 
            value={filePath} 
            readOnly 
            placeholder="Select Excel File (.xlsx)" 
            style={{ flexGrow: 1, padding: '5px' }} 
            />
            <button onClick={handleSelectFile} disabled={isProcessing}>Browse</button>
        </div>
        <div className="checkbox-wrapper">
    <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
        <input 
            type="checkbox" 
            checked={isBh} 
            onChange={(e) => set_isBh(e.target.checked)} 
        />
        Beyond Hello
    </label>
</div>
        <button 
            onClick={handleStart} 
            disabled={isProcessing || !filePath} 
            style={{ padding: '10px 20px', fontWeight: 'bold', width: '50%', margin: '20px' }}
        >
            {isProcessing ? 'IMPORTING...' : 'START IMPORT'}
        </button>
        <button 
            style={{ margin: '20px' }}
            onClick={() => showSettings ? setShowSettings(false) : openSettings()}
            >
                {showSettings ? '❌ Close Settings' : '⚙️ Settings'}
            </button>

            {showSettings && (
                <div className="settings-panel">
                    <h3
                    style={{ margin: '5px' }}
                    >Dutchie Credentials</h3>
                    <input 
                        type="text" 
                        placeholder="Username" 
                        value={username} 
                        onChange={(e) => setUsername(e.target.value)}
                        style={{ padding: '5px 10px', width: '70%', margin: '10px' }} 
                    />
                    <input 
                        type="password" 
                        placeholder="Password" 
                        value={password} 
                        onChange={(e) => setPassword(e.target.value)} 
                        style={{ padding: '5px 10px', width: '80%', margin: '10px' }} 
                    />
                    <button onClick={saveSettings}>Save Credentials</button>
                </div>
                )}
        <div style={{ marginTop: '20px' }}>
            <p style={{ textAlign: 'center' }}>{status}</p>
            <progress value={progress} max="100" style={{ width: '100%', height: '20px' }} />
        </div>
        </div>
            </main>
        
    );
}

async function checkForAppUpdates() {
    try {
        console.log("Checking for updates...");
        
        const update = await check();

        if (!update) {
            console.log("App is up to date.");
            return; 
        }

        const userWantsToUpdate = await confirm(
            `Packagie ${update.version} is available!\n\nRelease Notes:\n${update.body}\n\nDo you want to download and install it now?`, 
            {
                title: 'Update Available',
                kind: 'info',
                okLabel: 'Update Now',
                cancelLabel: 'Remind Me Later'
            }
        );

        if (userWantsToUpdate) {
            console.log("Downloading and installing update...");
            
            await update.downloadAndInstall((event) => {
                switch (event.event) {
                    case 'Started':
                        console.log(`Started downloading ${event.data.contentLength} bytes`);
                        break;
                    case 'Progress':
                        console.log(`Downloaded ${event.data.chunkLength} bytes`);
                        break;
                    case 'Finished':
                        console.log('Download finished');
                        break;
                }
            });

            console.log("Install complete. Relaunching...");
            
            await relaunch();
        }
        
    } catch (error: any) {
        console.error("Failed to check for updates:", error);
        await message(`Failed to update Packagie: ${error.message}`, {
            title: 'Update Error',
            kind: 'error'
        });
    }
}

checkForAppUpdates();