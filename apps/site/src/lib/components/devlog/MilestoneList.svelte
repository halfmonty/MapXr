<script lang="ts">
  const milestones = [
    { id: 1, title: 'Core types & JSON schema',    done: true  },
    { id: 2, title: 'Profile validation',          done: true  },
    { id: 3, title: 'LayerStack & SequenceEngine', done: true  },
    { id: 4, title: 'ComboEngine',                 done: true  },
    { id: 5, title: 'BLE scanner (tap-ble)',        done: true  },
    { id: 6, title: 'Tauri shell & commands',       done: true  },
    { id: 7, title: 'Svelte UI',                   done: false },
    { id: 8, title: 'tap-cli binary',              done: false },
    { id: 9, title: 'Packaging & CI',              done: false },
  ];
</script>

<div class="card bg-base-200 shadow-sm">
  <div class="card-body">
    <h3 class="card-title mb-3">Epic Progress</h3>
    <ul class="space-y-2">
      {#each milestones as m}
        <li class="flex items-center gap-3">
          <input
            type="checkbox"
            class="checkbox checkbox-primary checkbox-sm"
            checked={m.done}
            disabled
            aria-label={m.title}
          />
          <span class="text-sm {m.done ? 'line-through text-base-content/40' : ''}">
            {m.id}. {m.title}
          </span>
        </li>
      {/each}
    </ul>
  </div>
</div>
