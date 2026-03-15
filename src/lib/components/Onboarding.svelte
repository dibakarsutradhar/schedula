<script>
  import { createEventDispatcher, onMount } from 'svelte'
  import { fly, fade } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import DaySelect from './DaySelect.svelte'
  import { session } from '../stores/session.js'
  import { isSuperAdmin } from '../stores/session.js'
  import {
    updateDisplayName, changePassword,
    getOrganizations, createOrganization, updateOrganization,
    createLecturer, createRoom, createCourse,
    setupRecovery,
  } from '../api.js'

  const dispatch = createEventDispatcher()

  // ── Step state ───────────────────────────────────────────────────────────────
  let step = 0
  let dir = 1   // +1 = forward, -1 = backward
  let saving = false
  let error = ''

  // Step 1 — Account
  let displayName = $session?.display_name ?? ''
  let currentPwd = ''
  let newPwd = ''
  let confirmPwd = ''

  // Step 2 — Organization
  let orgName = ''
  let orgType = 'university'
  let contactEmail = ''
  let existingOrgId = null

  // Step 3 — Lecturer
  let lecturerName = ''
  let lecturerDays = 'Mon,Tue,Wed,Thu,Fri'

  // Step 4 — Room
  let roomName = ''
  let roomType = 'lecture'
  let roomCapacity = 30

  // Step 5 — Course
  let courseCode = ''
  let courseName = ''
  let courseHours = 3
  let courseClassType = 'lecture'

  // Step 6 — Recovery Setup (super-admin only)
  let securityQuestion = 'What is your favorite university city?'
  let securityAnswer = ''
  let recoveryCode = ''  // displayed after setup
  let showRecoveryCode = false

  // Completion tracking
  let created = { account: false, org: false, lecturer: false, room: false, course: false, recovery: false }

  const TOTAL_FORM_STEPS = 6  // now includes recovery setup

  // ── Load existing org on mount ────────────────────────────────────────────────
  onMount(async () => {
    try {
      const orgs = await getOrganizations()
      const myOrg = isSuperAdmin($session) ? orgs[0] : orgs.find(o => o.id === $session?.org_id)
      if (myOrg) {
        orgName = myOrg.name
        orgType = myOrg.org_type
        contactEmail = myOrg.contact_email ?? ''
        existingOrgId = myOrg.id
      }
    } catch (_) {}
  })

  // ── Transition helper ─────────────────────────────────────────────────────────
  // Captured at transition time — must read dir as a closure
  function inFx()  { return { x: dir * 48,  duration: 300, easing: cubicOut, delay: 40 } }
  function outFx() { return { x: -dir * 48, duration: 220, easing: cubicOut } }

  // ── Navigation ────────────────────────────────────────────────────────────────
  async function goNext() {
    error = ''
    saving = true
    try {
      await saveStep(step)
      dir = 1; step++
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  function goBack() {
    error = ''
    dir = -1; step--
  }

  function skip() {
    error = ''
    dir = 1; step++
  }

  async function finish() {
    saving = true; error = ''
    try {
      await saveStep(step)
      markDone()
    } catch (e) {
      error = String(e); saving = false
    }
  }

  // ── Save logic per step ───────────────────────────────────────────────────────
  async function saveStep(s) {
    if (s === 1) {
      if (displayName.trim() && displayName.trim() !== $session?.display_name) {
        await updateDisplayName(displayName.trim())
        session.set({ ...$session, display_name: displayName.trim() })
        created.account = true
      }
      if (newPwd) {
        if (newPwd !== confirmPwd) throw new Error('Passwords do not match')
        if (newPwd.length < 6)    throw new Error('Password must be at least 6 characters')
        if (!currentPwd)          throw new Error('Enter your current password to change it')
        await changePassword(currentPwd, newPwd)
        currentPwd = ''; newPwd = ''; confirmPwd = ''
        created.account = true
      }
    }
    if (s === 2 && orgName.trim()) {
      const payload = { name: orgName.trim(), org_type: orgType, contact_email: contactEmail.trim() || null, address: null }
      if (existingOrgId) {
        await updateOrganization(existingOrgId, payload)
      } else {
        existingOrgId = await createOrganization(payload)
      }
      created.org = true
    }
    if (s === 3 && lecturerName.trim()) {
      await createLecturer({ name: lecturerName.trim(), email: null, available_days: lecturerDays, max_hours_per_day: 4, max_hours_per_week: 16, org_id: $session?.org_id ?? existingOrgId ?? null })
      created.lecturer = true
    }
    if (s === 4 && roomName.trim()) {
      await createRoom({ name: roomName.trim(), capacity: +roomCapacity, room_type: roomType, available_days: 'Mon,Tue,Wed,Thu,Fri', org_id: $session?.org_id ?? existingOrgId ?? null })
      created.room = true
    }
    if (s === 5 && courseCode.trim() && courseName.trim()) {
      await createCourse({ code: courseCode.trim(), name: courseName.trim(), hours_per_week: +courseHours, room_type: courseClassType === 'lab' ? 'lab' : 'lecture', class_type: courseClassType, frequency: 'weekly', lecturer_id: null, org_id: $session?.org_id ?? existingOrgId ?? null })
      created.course = true
    }
    if (s === 6 && isSuperAdmin($session) && securityQuestion.trim() && securityAnswer.trim()) {
      const result = await setupRecovery({ security_question: securityQuestion.trim(), security_answer: securityAnswer.trim() })
      recoveryCode = result.recovery_code
      showRecoveryCode = true
      created.recovery = true
    }
  }

  function markDone() {
    localStorage.setItem(`schedula_onboarded_${$session?.user_id}`, '1')
    dispatch('complete', { navigateTo: 'batches' })
  }

  // Step labels/icons for 1–6
  const stepMeta = [
    null,
    { num: 1, icon: '👤', title: 'Set up your account'  },
    { num: 2, icon: '🏫', title: 'Your organization'    },
    { num: 3, icon: '🎓', title: 'Add a lecturer'       },
    { num: 4, icon: '🏛',  title: 'Add a room'           },
    { num: 5, icon: '📚', title: 'Add a course'         },
    { num: 6, icon: '🔐', title: 'Password recovery'    },
  ]
</script>

<!-- ─── Backdrop ──────────────────────────────────────────────────────────────── -->
<div class="ob-backdrop" transition:fade={{ duration: 280 }}>
  <div class="ob-card">

    <!-- Step 0: Welcome ─────────────────────────────────────────────────────── -->
    {#if step === 0}
      {#key step}
        <div class="ob-welcome" in:fly={inFx()} out:fly={outFx()}>
          <div class="ob-brand">
            <span class="ob-brand-icon">◈</span>
            <span class="ob-brand-name">Schedula</span>
          </div>
          <h1 class="ob-welcome-title">Welcome to Schedula</h1>
          <p class="ob-welcome-sub">
            Your AI-powered timetable generator.<br>
            Let's set up your workspace — it only takes a few minutes.
          </p>

          <div class="ob-feature-grid">
            {#each [
              ['⚡', 'Instant Schedules', 'Generate conflict-free timetables in seconds'],
              ['🔒', 'Zero Conflicts', '7 hard constraints guarantee no overlaps'],
              ['🎨', 'Beautiful Views', 'Grid, list, and semester calendar views'],
              ['🏢', 'Multi-Org Ready', 'Manage departments and organizations'],
            ] as [icon, title, desc]}
              <div class="ob-feature">
                <span class="ob-feature-icon">{icon}</span>
                <div>
                  <strong>{title}</strong>
                  <p>{desc}</p>
                </div>
              </div>
            {/each}
          </div>

          <div class="ob-welcome-actions">
            <button class="btn btn-primary ob-btn-lg" on:click={() => { dir = 1; step = 1 }}>
              Get Started →
            </button>
            <button class="ob-skip-link" on:click={markDone}>
              Already set up? Skip
            </button>
          </div>
        </div>
      {/key}

    <!-- Step 6: Recovery Setup ───────────────────────────────────────────────── -->
    {:else if step === 6 && isSuperAdmin($session)}
      {#key step}
        <div class="ob-step-body ob-recovery" in:fly={inFx()} out:fly={outFx()}>
          <p class="ob-hint">
            Set up password recovery so you can access your account if you forget your password.
          </p>

          {#if !showRecoveryCode}
            <div class="form-group">
              <label class="form-label">Security Question *</label>
              <input class="form-input" bind:value={securityQuestion} placeholder="e.g. What city were you born in?" autofocus />
            </div>
            <div class="form-group">
              <label class="form-label">Your Answer *</label>
              <input class="form-input" bind:value={securityAnswer} placeholder="Answer to your question" />
            </div>
          {:else}
            <div class="recovery-code-box">
              <div class="recovery-header">
                <span style="font-size:24px">🔐</span>
                <div>
                  <strong>Recovery Code Generated</strong>
                  <p style="font-size:12px;color:var(--text-muted);margin:0">Write this down and keep it safe</p>
                </div>
              </div>
              <div class="recovery-code-display">{recoveryCode}</div>
              <p style="font-size:12px;color:var(--danger);margin-top:12px">
                ⚠️ This code will not be shown again. Write it down now.
              </p>
            </div>
          {/if}

          {#if error}
            <div class="ob-error" in:fly={{ y: -8, duration: 200 }}>{error}</div>
          {/if}
        </div>
      {/key}

    <!-- Step 7: All Done! ────────────────────────────────────────────────────── -->
    {:else if step === 7}
      {#key step}
        <div class="ob-done" in:fly={inFx()} out:fly={outFx()}>
          <div class="ob-done-icon">🎉</div>
          <h2>You're all set!</h2>
          <p class="ob-done-sub">
            Schedula is ready to generate your first conflict-free timetable.
          </p>

          {#if Object.values(created).some(Boolean)}
            <div class="ob-summary">
              {#each [
                [created.account || (displayName && displayName !== ($session?.display_name ?? '')), '👤', 'Account configured'],
                [created.org, '🏫', `Organization: ${orgName}`],
                [created.lecturer, '🎓', `Lecturer: ${lecturerName}`],
                [created.room, '🏛', `Room: ${roomName}`],
                [created.course, '📚', `Course: ${courseCode} — ${courseName}`],
                [created.recovery, '🔐', 'Password recovery set up'],
              ] as [done, icon, label]}
                {#if done}
                  <div class="ob-summary-item">
                    <span class="ob-check">✓</span>
                    <span>{icon}</span>
                    <span>{label}</span>
                  </div>
                {/if}
              {/each}
            </div>
          {/if}

          <div class="ob-done-tip">
            <strong>Next:</strong> Add student batches in the Batches tab, then hit Generate Schedule.
          </div>

          <div class="ob-done-actions">
            <button class="btn btn-primary ob-btn-lg" on:click={markDone}>
              Start Scheduling →
            </button>
            <button class="ob-skip-link" on:click={markDone}>Go to dashboard</button>
          </div>
        </div>
      {/key}

    <!-- Steps 1–5: Form steps ─────────────────────────────────────────────────── -->
    {:else}
      <!-- Progress dots (static, no transition) -->
      <div class="ob-progress">
        {#each Array(TOTAL_FORM_STEPS) as _, i}
          <div class="ob-dot" class:active={i === step - 1} class:done={i < step - 1}></div>
        {/each}
      </div>

      <!-- Step header (transitions with step changes) -->
      {#key step}
        <div class="ob-step-header" in:fly={inFx()} out:fly={outFx()}>
          <div class="ob-step-icon">{stepMeta[step]?.icon}</div>
          <div>
            <div class="ob-step-num">Step {stepMeta[step]?.num} of {TOTAL_FORM_STEPS}</div>
            <h2 class="ob-step-title">{stepMeta[step]?.title}</h2>
          </div>
        </div>
      {/key}

      <!-- Step body (transitions with step changes) -->
      {#key step}
        <div class="ob-step-body" in:fly={inFx()} out:fly={outFx()}>

          <!-- Step 1: Account ──────────────────────────────────────────────── -->
          {#if step === 1}
            <div class="form-group">
              <label class="form-label">Your Display Name *</label>
              <input class="form-input" bind:value={displayName} placeholder="Dr. Sarah Ahmed" autofocus />
            </div>

            <div class="ob-divider"><span>Change Password <span class="ob-optional">(recommended)</span></span></div>

            <div class="form-group">
              <label class="form-label">Current Password</label>
              <input class="form-input" type="password" bind:value={currentPwd} placeholder="Default is admin123" />
            </div>
            <div class="row">
              <div class="form-group">
                <label class="form-label">New Password</label>
                <input class="form-input" type="password" bind:value={newPwd} placeholder="Min 6 characters" />
              </div>
              <div class="form-group">
                <label class="form-label">Confirm Password</label>
                <input class="form-input" type="password" bind:value={confirmPwd} />
              </div>
            </div>

          <!-- Step 2: Organization ──────────────────────────────────────────── -->
          {:else if step === 2}
            <div class="form-group">
              <label class="form-label">Institution Name *</label>
              <input class="form-input" bind:value={orgName} placeholder="University of Dhaka" autofocus />
            </div>
            <div class="row">
              <div class="form-group">
                <label class="form-label">Type</label>
                <select class="form-select" bind:value={orgType}>
                  <option value="university">🎓 University</option>
                  <option value="college">🏫 College</option>
                  <option value="school">📖 School</option>
                  <option value="institute">🔬 Institute</option>
                </select>
              </div>
              <div class="form-group">
                <label class="form-label">Contact Email <span class="ob-optional">(optional)</span></label>
                <input class="form-input" type="email" bind:value={contactEmail} placeholder="admin@university.edu" />
              </div>
            </div>

          <!-- Step 3: Lecturer ──────────────────────────────────────────────── -->
          {:else if step === 3}
            <p class="ob-hint">Lecturers are assigned to courses. You can add more anytime from the Lecturers tab.</p>
            <div class="form-group">
              <label class="form-label">Full Name</label>
              <input class="form-input" bind:value={lecturerName} placeholder="Dr. Sarah Ahmed" autofocus />
            </div>
            <div class="form-group">
              <label class="form-label">Available Days</label>
              <DaySelect bind:value={lecturerDays} />
            </div>

          <!-- Step 4: Room ──────────────────────────────────────────────────── -->
          {:else if step === 4}
            <p class="ob-hint">Rooms host your classes. The scheduler assigns rooms by type and capacity.</p>
            <div class="row">
              <div class="form-group" style="flex:2">
                <label class="form-label">Room Name</label>
                <input class="form-input" bind:value={roomName} placeholder="A-101" autofocus />
              </div>
              <div class="form-group">
                <label class="form-label">Capacity</label>
                <input class="form-input" type="number" min="1" bind:value={roomCapacity} />
              </div>
            </div>
            <div class="form-group">
              <label class="form-label">Room Type</label>
              <div class="ob-radio-group">
                {#each [['lecture','🏛','Lecture Room','Standard classes'], ['lab','🔬','Lab','Practical sessions']] as [val, icon, label, desc]}
                  <label class="ob-radio-card" class:active={roomType === val}>
                    <input type="radio" bind:group={roomType} value={val} />
                    <span class="ob-radio-icon">{icon}</span>
                    <div><strong>{label}</strong><small>{desc}</small></div>
                  </label>
                {/each}
              </div>
            </div>

          <!-- Step 5: Course ────────────────────────────────────────────────── -->
          {:else if step === 5}
            <p class="ob-hint">Courses are assigned to student batches. Add more later from the Courses tab.</p>
            <div class="row">
              <div class="form-group">
                <label class="form-label">Course Code</label>
                <input class="form-input" bind:value={courseCode} placeholder="CS-201" autofocus />
              </div>
              <div class="form-group" style="flex:2">
                <label class="form-label">Course Name</label>
                <input class="form-input" bind:value={courseName} placeholder="Data Structures" />
              </div>
            </div>
            <div class="row">
              <div class="form-group">
                <label class="form-label">Hours / Week</label>
                <input class="form-input" type="number" min="1" max="10" bind:value={courseHours} />
              </div>
              <div class="form-group">
                <label class="form-label">Class Type</label>
                <select class="form-select" bind:value={courseClassType}>
                  <option value="lecture">Lecture</option>
                  <option value="lab">Lab</option>
                  <option value="tutorial">Tutorial</option>
                </select>
              </div>
            </div>
          {/if}

          <!-- Error message -->
          {#if error}
            <div class="ob-error" in:fly={{ y: -8, duration: 200 }}>{error}</div>
          {/if}
        </div>
      {/key}

      <!-- Footer (static, no transition) -->
      <div class="ob-footer">
        <button class="btn btn-secondary" on:click={goBack} disabled={saving}>← Back</button>
        <div class="ob-footer-right">
          <button class="ob-skip-link" on:click={skip} disabled={saving}>Skip this step</button>
          {#if step < TOTAL_FORM_STEPS}
            <button
              class="btn btn-primary"
              on:click={goNext}
              disabled={saving || (step === 1 && !displayName.trim()) || (step === 2 && !orgName.trim()) || (step === 6 && (!showRecoveryCode || !securityQuestion.trim() || !securityAnswer.trim()))}
            >{saving ? 'Saving…' : 'Continue →'}</button>
          {:else if step === TOTAL_FORM_STEPS}
            <button class="btn btn-primary" on:click={finish} disabled={saving}>
              {saving ? 'Saving…' : 'Finish Setup →'}
            </button>
          {/if}
        </div>
      </div>
    {/if}

  </div>
</div>

<style>
  /* ── Backdrop ─────────────────────────────────────────────────────────────── */
  .ob-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    padding: 20px;
  }

  /* ── Card ─────────────────────────────────────────────────────────────────── */
  .ob-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 20px;
    width: 600px;
    max-width: 100%;
    max-height: 92vh;
    overflow-y: auto;
    overflow-x: hidden;
    box-shadow: 0 40px 100px rgba(0, 0, 0, 0.65);
    position: relative;
  }

  /* ── Welcome screen ───────────────────────────────────────────────────────── */
  .ob-welcome {
    padding: 48px 44px 44px;
    text-align: center;
  }

  .ob-brand {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    margin-bottom: 28px;
  }
  .ob-brand-icon {
    font-size: 38px;
    color: var(--accent);
    filter: drop-shadow(0 0 20px var(--accent));
    animation: pulse 2s ease-in-out infinite;
  }
  .ob-brand-name {
    font-size: 30px;
    font-weight: 800;
    letter-spacing: -0.04em;
    background: linear-gradient(135deg, var(--accent) 0%, var(--accent2) 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .ob-welcome-title {
    font-size: 1.55rem;
    font-weight: 700;
    margin-bottom: 12px;
    letter-spacing: -0.025em;
  }
  .ob-welcome-sub {
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1.75;
    margin-bottom: 32px;
  }

  .ob-feature-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-bottom: 36px;
    text-align: left;
  }
  .ob-feature {
    display: flex;
    gap: 11px;
    align-items: flex-start;
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px;
    transition: border-color .2s;
  }
  .ob-feature:hover { border-color: rgba(108,99,255,.4); }
  .ob-feature-icon { font-size: 20px; flex-shrink: 0; margin-top: 1px; }
  .ob-feature strong { display: block; font-size: 13px; margin-bottom: 3px; }
  .ob-feature p { font-size: 12px; color: var(--text-muted); margin: 0; line-height: 1.5; }

  .ob-welcome-actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }

  /* ── Done screen ──────────────────────────────────────────────────────────── */
  .ob-done {
    padding: 48px 44px 44px;
    text-align: center;
  }
  .ob-done-icon {
    font-size: 60px;
    margin-bottom: 20px;
    display: block;
    animation: pop .5s cubic-bezier(.175,.885,.32,1.275) both;
  }
  .ob-done h2 { font-size: 1.55rem; font-weight: 700; margin-bottom: 10px; }
  .ob-done-sub { color: var(--text-muted); font-size: 14px; line-height: 1.75; margin-bottom: 24px; }

  .ob-summary {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 20px;
    text-align: left;
  }
  .ob-summary-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: var(--surface2);
    border-radius: 8px;
    border: 1px solid var(--border);
    font-size: 13px;
    animation: slideIn .3s ease both;
  }
  .ob-check { color: var(--success); font-weight: 700; font-size: 15px; flex-shrink: 0; }

  .ob-done-tip {
    background: rgba(108,99,255,.08);
    border: 1px solid rgba(108,99,255,.2);
    border-radius: 10px;
    padding: 12px 16px;
    font-size: 13px;
    color: var(--text-muted);
    margin-bottom: 28px;
    text-align: left;
    line-height: 1.6;
  }
  .ob-done-tip strong { color: var(--accent2); }

  .ob-done-actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }

  /* ── Progress dots ────────────────────────────────────────────────────────── */
  .ob-progress {
    display: flex;
    justify-content: center;
    gap: 8px;
    padding: 28px 44px 0;
  }
  .ob-dot {
    width: 8px; height: 8px;
    border-radius: 50%;
    background: var(--border);
    transition: all .35s cubic-bezier(.4,0,.2,1);
    flex-shrink: 0;
  }
  .ob-dot.done { background: var(--accent); opacity: 0.45; width: 8px; }
  .ob-dot.active {
    background: var(--accent);
    width: 28px;
    border-radius: 4px;
    box-shadow: 0 0 10px rgba(108,99,255,.6);
  }

  /* ── Step header ──────────────────────────────────────────────────────────── */
  .ob-step-header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 22px 44px 4px;
  }
  .ob-step-icon { font-size: 34px; flex-shrink: 0; }
  .ob-step-num {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: .06em;
    margin-bottom: 4px;
  }
  .ob-step-title {
    font-size: 1.3rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 0;
  }

  /* ── Step body ────────────────────────────────────────────────────────────── */
  .ob-step-body {
    padding: 22px 44px 8px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 240px;
  }

  .ob-hint {
    font-size: 13px;
    color: var(--text-muted);
    background: var(--surface2);
    border-radius: 8px;
    padding: 10px 14px;
    border-left: 3px solid var(--accent);
    margin: 0;
    line-height: 1.6;
  }

  .ob-divider {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: .04em;
  }
  .ob-divider::before, .ob-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border);
  }
  .ob-optional { font-size: 11px; color: var(--text-muted); font-weight: 400; opacity: .7; }

  .ob-radio-group { display: flex; gap: 12px; }
  .ob-radio-card {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 2px solid var(--border);
    border-radius: 10px;
    cursor: pointer;
    transition: all .2s;
    background: var(--surface2);
    user-select: none;
  }
  .ob-radio-card:hover { border-color: rgba(108,99,255,.5); }
  .ob-radio-card.active {
    border-color: var(--accent);
    background: rgba(108,99,255,.08);
    box-shadow: 0 0 0 3px rgba(108,99,255,.12);
  }
  .ob-radio-card input { display: none; }
  .ob-radio-icon { font-size: 22px; flex-shrink: 0; }
  .ob-radio-card strong { display: block; font-size: 13px; margin-bottom: 2px; }
  .ob-radio-card small { font-size: 12px; color: var(--text-muted); }

  .ob-error {
    background: rgba(239,68,68,.1);
    border: 1px solid rgba(239,68,68,.3);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 13px;
    color: #f87171;
  }

  /* ── Footer ───────────────────────────────────────────────────────────────── */
  .ob-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 44px 32px;
    gap: 12px;
    border-top: 1px solid var(--border);
    margin-top: 8px;
  }
  .ob-footer-right { display: flex; align-items: center; gap: 16px; }

  /* ── Shared ───────────────────────────────────────────────────────────────── */
  .ob-btn-lg {
    padding: 13px 32px;
    font-size: 14px;
    font-weight: 600;
    border-radius: 12px;
    min-width: 180px;
  }
  .ob-skip-link {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    padding: 4px 0;
    transition: color .15s;
  }
  .ob-skip-link:hover { color: var(--text); }
  .ob-skip-link:disabled { opacity: .4; cursor: not-allowed; }

  /* ── Animations ───────────────────────────────────────────────────────────── */
  @keyframes pulse {
    0%, 100% { filter: drop-shadow(0 0 12px var(--accent)); }
    50%       { filter: drop-shadow(0 0 24px var(--accent)); }
  }
  @keyframes pop {
    0%   { transform: scale(0.4); opacity: 0; }
    70%  { transform: scale(1.15); }
    100% { transform: scale(1);   opacity: 1; }
  }
  @keyframes slideIn {
    from { opacity: 0; transform: translateX(-8px); }
    to   { opacity: 1; transform: translateX(0); }
  }

  /* ── Recovery setup ───────────────────────────────────────────────────────── */
  .ob-recovery { min-height: 320px; }
  .recovery-code-box {
    background: rgba(34,197,94,.08);
    border: 2px solid #4ade80;
    border-radius: 12px;
    padding: 16px;
  }
  .recovery-header {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
  }
  .recovery-header strong { font-size: 14px; display: block; }
  .recovery-code-display {
    background: var(--surface2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 14px;
    font-family: monospace;
    font-size: 14px;
    letter-spacing: 0.05em;
    word-break: break-all;
    color: var(--accent);
    font-weight: 700;
    user-select: all;
  }

  /* ── Light theme ──────────────────────────────────────────────────────────── */
  :global([data-theme="light"]) .ob-card {
    box-shadow: 0 40px 100px rgba(0,0,0,.18);
  }
</style>
