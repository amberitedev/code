import { defineStore } from 'pinia'
import { supabase } from '@/plugins/supabase'

export const useAuthStore = defineStore('auth', {
  state: () => ({
    user: null,
    session: null,
    loading: false,
    error: null
  }),

  getters: {
    isAuthenticated: (state) => !!state.session
  },

  actions: {
    async init() {
      this.loading = true
      this.error = null
      try {
        const { data, error } = await supabase.auth.getSession()
        if (error) throw error
        this.session = data.session
        this.user = data.user
      } catch (err) {
        this.error = err.message
      } finally {
        this.loading = false
      }
    },

    async signIn(email, password) {
      this.loading = true
      this.error = null
      try {
        const { data, error } = await supabase.auth.signInWithPassword({ email, password })
        if (error) throw error
        this.session = data.session
        this.user = data.user
        return { success: true }
      } catch (err) {
        this.error = err.message
        return { success: false, error: err.message }
      } finally {
        this.loading = false
      }
    },

    async signUp(email, password) {
      this.loading = true
      this.error = null
      try {
        const { data, error } = await supabase.auth.signUp({ email, password })
        if (error) throw error
        this.user = data.user
        this.session = data.session
        return { success: true }
      } catch (err) {
        this.error = err.message
        return { success: false, error: err.message }
      } finally {
        this.loading = false
      }
    },

    async signOut() {
      this.loading = true
      this.error = null
      try {
        const { error } = await supabase.auth.signOut()
        if (error) throw error
        this.session = null
        this.user = null
        return { success: true }
      } catch (err) {
        this.error = err.message
        return { success: false, error: err.message }
      } finally {
        this.loading = false
      }
    },

    async resetPassword(email) {
      this.loading = true
      this.error = null
      try {
        const { error } = await supabase.auth.resetPasswordForEmail(email)
        if (error) throw error
        return { success: true }
      } catch (err) {
        this.error = err.message
        return { success: false, error: err.message }
      } finally {
        this.loading = false
      }
    },

    async updateProfile(updates) {
      this.loading = true
      this.error = null
      try {
        const { data, error } = await supabase.auth.updateUser(updates)
        if (error) throw error
        this.user = data.user
        return { success: true }
      } catch (err) {
        this.error = err.message
        return { success: false, error: err.message }
      } finally {
        this.loading = false
      }
    }
  }
})
