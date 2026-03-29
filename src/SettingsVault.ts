import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';

export const SecureStore = {
    async getKey(usernameSeed: string) {
        const enc = new TextEncoder();
        const keyMaterial = await crypto.subtle.importKey(
            "raw", enc.encode(usernameSeed), { name: "PBKDF2" }, false, ["deriveKey"]
        );
        return await crypto.subtle.deriveKey(
            { name: "PBKDF2", salt: enc.encode("packagie_salt"), iterations: 100000, hash: "SHA-256" },
            keyMaterial, { name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]
        );
    },

    async encrypt(plainText: string, usernameSeed: string) {
        if (!plainText) return "";
        const key = await this.getKey(usernameSeed);
        const iv = crypto.getRandomValues(new Uint8Array(12));
        const enc = new TextEncoder();
        const encrypted = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, enc.encode(plainText));
        const combined = new Uint8Array(iv.length + encrypted.byteLength);
        combined.set(iv);
        combined.set(new Uint8Array(encrypted), iv.length);
        return btoa(String.fromCharCode(...combined));
    },

    async decrypt(cipherText: string, usernameSeed: string) {
        if (!cipherText) return "";
        try {
            const key = await this.getKey(usernameSeed);
            const combined = new Uint8Array(atob(cipherText).split('').map(c => c.charCodeAt(0)));
            const iv = combined.slice(0, 12);
            const data = combined.slice(12);
            const decrypted = await crypto.subtle.decrypt({ name: "AES-GCM", iv }, key, data);
            return new TextDecoder().decode(decrypted);
        } catch (e) {
            console.warn("Could not decrypt password. Username may have changed.");
            return ""; 
        }
    }
};

export async function triggerAutoLogin() {
    try {
        const store = await load('settings.json');
        const osUser = await invoke<string>('get_os_username');
        
        const savedUsername = await store.get<string>('username');
        const savedEncryptedPassword = await store.get<string>('password');

        if (savedUsername && savedEncryptedPassword) {
            const decryptedPass = await SecureStore.decrypt(savedEncryptedPassword, osUser);
            if (decryptedPass) {
                console.log("Credentials found. Attempting auto-login...");
                await invoke('auto_login', { user: savedUsername, pass: decryptedPass });
            }
        }
    } catch (error) {
        console.error("Failed to execute auto-login:", error);
    }
}