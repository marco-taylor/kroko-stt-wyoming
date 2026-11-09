#!/usr/bin/env python3
"""Validate the public Unraid contract without Docker or network access."""
import pathlib
import unittest
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]


class TemplateContract(unittest.TestCase):
    def setUp(self):
        self.template = ET.parse(ROOT / 'templates/kroko-stt-wyoming.xml').getroot()
        self.config = self.template.findall('Config')

    def test_canonical_names_and_links(self):
        self.assertEqual(self.template.findtext('Name'), 'kroko-stt-wyoming')
        self.assertEqual(self.template.findtext('Repository'),
                         'ghcr.io/marco-taylor/kroko-stt-wyoming:latest')
        for tag in ['Registry', 'Support', 'Project', 'ReadMe', 'TemplateURL', 'Icon']:
            # Legacy-name regression: current public links must use canonical names.
            self.assertNotIn('kroko-stt-german-wyoming', self.template.findtext(tag))

    def test_legacy_icon_alias_preserves_saved_template_links(self):
        # Legacy compatibility for saved Unraid templates; not another app XML.
        legacy = ROOT / 'icons/kroko-stt-german-wyoming.png'
        canonical = ROOT / 'icons/kroko-stt-wyoming.png'
        self.assertEqual(legacy.read_bytes(), canonical.read_bytes())

    def test_internal_constants_are_not_user_options(self):
        variables = {c.get('Target') for c in self.config if c.get('Type') == 'Variable'}
        self.assertTrue(variables.isdisjoint({'HOST', 'PORT', 'MODEL_DIR'}))
        dockerfile = (ROOT / 'Dockerfile').read_text()
        for setting in ['HOST=0.0.0.0', 'PORT=10321', 'MODEL_DIR=/models']:
            self.assertIn(setting, dockerfile)

    def test_only_host_port_and_model_mount_are_exposed(self):
        ports = [c for c in self.config if c.get('Type') == 'Port']
        self.assertEqual(len(ports), 1)
        self.assertEqual(ports[0].get('Target'), '10321')
        self.assertEqual(ports[0].get('Mode'), 'tcp')
        paths = [c for c in self.config if c.get('Type') == 'Path']
        self.assertEqual(len(paths), 1)
        self.assertEqual(paths[0].get('Target'), '/models')
        self.assertEqual(paths[0].text, '/mnt/user/appdata/kroko-stt-wyoming/models')

    def test_language_options_match_real_catalog(self):
        import json
        catalog = json.loads((ROOT / 'src/server/model_catalog.json').read_text())
        expected = {m['language'] for m in catalog['models']}
        language = next(c for c in self.config if c.get('Target') == 'LANGUAGE')
        self.assertEqual(set(language.get('Default').split('|')), expected)
        self.assertEqual(language.text, 'de')
        self.assertNotIn('Ausschließlich Deutsch', language.get('Description'))

    def test_exactly_one_app_and_matching_template_url(self):
        templates = sorted((ROOT / 'templates').glob('*.xml'))
        self.assertEqual([p.name for p in templates], ['kroko-stt-wyoming.xml'])
        for path in templates:
            template = ET.parse(path).getroot()
            self.assertEqual(template.tag, 'Container')
            self.assertEqual(template.get('version'), '2')
            self.assertEqual(template.findtext('TemplateURL'),
                             'https://raw.githubusercontent.com/marco-taylor/'
                             'kroko-stt-wyoming/main/templates/' + path.name)

    def test_repository_profile_is_present(self):
        profile = ET.parse(ROOT / 'ca_profile.xml').getroot()
        self.assertEqual(profile.tag, 'CommunityApplications')
        self.assertTrue(profile.findtext('Profile', '').strip())
        self.assertTrue((ROOT / 'LICENSE').is_file())

    def test_tailscale_is_not_added_by_this_project(self):
        self.assertFalse(any(c.get('Target') == 'CA_TS_FALLBACK_DIR' for c in self.config))


if __name__ == '__main__':
    unittest.main()
