#include <lib/std.mi>
#include <lib/fileio.mi>
#define DEF_MAX 50

Global Container player;
Global Layout shade, normal;

Global Group temp_g, tagViewer; //frameGroup, 
Global XmlDoc myDoc;
Global Text textObject;



System.onScriptLoaded (){
	//frameGroup = getScriptGroup ();
	
	player = System.getContainer("main");
	normal = player.getLayout("normal");
	shade = player.getLayout("shade");
	
	myDoc = new XmlDoc;
	String fullpath = getParam()+"ClassicPro.xml";
	myDoc.load (fullpath);
	
	// If we include more stuff in the classicpro.xml at a later stage the parser must set a boolean = true to know that the xml was for this
	if(myDoc.exists()){
		myDoc.parser_addCallback("ClassicPro/TextSettings*");
		myDoc.parser_start();
		myDoc.parser_destroy();
	}
	delete myDoc;


}

myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	if(strlower(xmltag) == "style"){
		String busyWith = "";
		String tabsettings = "";
		for(int i=0; i<paramname.getNumItems(); i++){
			if(strlower(paramname.enumItem(i))=="id"){
				busyWith=paramvalue.enumItem(i);
			}
			else if(busyWith=="normal.statusbar.text"){ //else if - because otherwise the ID param gets set too - might be buggy in cpro1 - check later (note to myself pjn)

				//Playlist status text
				temp_g = normal.findObject("centro.playlist2");
				textObject = temp_g.getObject("centro2.group.pl.buttons").findObject("centro.playlist.pltext1");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				temp_g = normal.findObject("drawer.playlist");
				textObject = temp_g.getObject("centro2.group.pl.buttons").findObject("centro.playlist.pltext1");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				temp_g = normal.findObject("centro.playlist.component");
				textObject = temp_g.getObject("centro2.group.pl.buttons").findObject("centro.playlist.pltext1");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				//Video status text
				temp_g = normal.findObject("centro.video");
				textObject = temp_g.getObject("centro.video.buttons").findObject("centro2.group.video.buttons.text");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				//File info status text
				temp_g = normal.findObject("centro.playlist.directory.tag");
				textObject = temp_g.findObject("tagviewer.status.box.text");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				temp_g = normal.findObject("centro.multidrawer");
				textObject = temp_g.findObject("tagviewer.status.box.text");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
				
				temp_g = normal.findObject("centro.visualization.buttons");
				textObject = temp_g.getObject("centro.visname");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));

				temp_g = NULL;
			}
			else if(busyWith=="normal.fileinfo.label.text"){
				tagViewer = normal.findObject("centro.multidrawer").findObject("info.component.infodisplay");
				tagViewer.sendAction("update_text_a", paramname.enumItem(i) + ";" + paramvalue.enumItem(i), 0, 0, 0, 0);
				tagViewer = normal.findObject("centro.playlist.directory").findObject("info.component.infodisplay");
				tagViewer.sendAction("update_text_a", paramname.enumItem(i) + ";" + paramvalue.enumItem(i), 0, 0, 0, 0);
			}		
			else if(busyWith=="normal.fileinfo.tag.text"){
				tagViewer = normal.findObject("centro.multidrawer").findObject("info.component.infodisplay");
				tagViewer.sendAction("update_text_b", paramname.enumItem(i) + ";" + paramvalue.enumItem(i), 0, 0, 0, 0);
				tagViewer = normal.findObject("centro.playlist.directory").findObject("info.component.infodisplay");
				tagViewer.sendAction("update_text_b", paramname.enumItem(i) + ";" + paramvalue.enumItem(i), 0, 0, 0, 0);
			}		
			
			else if(busyWith=="normal.info.title.text"){
				textObject = normal.findObject("two.info.text.title");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}		
			else if(busyWith=="normal.info.artist.text"){
				textObject = normal.findObject("two.info.text.artist");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}	
			else if(busyWith=="normal.info.timebig.text"){
				textObject = normal.findObject("two.info.text.tracktime");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}		
			else if(busyWith=="normal.info.timesmall.text"){
				textObject = normal.findObject("two.info.text.totaltime");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}		
			else if(busyWith=="normal.info.news.text"){
				textObject = normal.findObject("two.info.text.news");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}		
			else if(busyWith=="normal.info.bitrate.text"){
				textObject = normal.findObject("two.info.text.info.1");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="normal.webreader.text"){
				textObject = normal.findObject("browserpro.ddl.text");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="normal.tab.text"){
				tabsettings+= paramname.enumItem(i)+","+paramvalue.enumItem(i)+";";
			}
			
			
			else if(busyWith=="shade.songname.text"){
				textObject = shade.findObject("shade.st.title");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="shade.news.text"){
				textObject = shade.findObject("shade.st.news");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="shade.timebig.text"){
				textObject = shade.findObject("shade.text.time.tracktime");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="shade.timesmall.text"){
				textObject = shade.findObject("shade.text.time.totaltime");
				textObject.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}


		}
		
		if(busyWith=="normal.tab.text"){
			setPrivateString(getSkinName(), "tabtext", tabsettings);
			tabsettings="";
		}
	}
}